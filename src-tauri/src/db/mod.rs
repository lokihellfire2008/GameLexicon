use anyhow::{Context, Result};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    Pool, Sqlite,
};
use std::path::{Path, PathBuf};
use std::str::FromStr;

pub type Db = Pool<Sqlite>;

fn extract_path(database_url: &str) -> Option<String> {
    let mut rest = if let Some(r) = database_url.strip_prefix("sqlite:///") {
        r
    } else if let Some(r) = database_url.strip_prefix("sqlite://") {
        r
    } else if let Some(r) = database_url.strip_prefix("sqlite:") {
        r
    } else if database_url.contains(':')
        || database_url.contains('/')
        || database_url.contains('\\')
    {
        database_url
    } else {
        return None;
    };

    if let Some((p, _)) = rest.split_once('?') {
        rest = p;
    }

    #[cfg(windows)]
    {
        let b = rest.as_bytes();
        if b.len() >= 3 && b[0] == b'/' && b[1].is_ascii_alphabetic() && b[2] == b':' {
            rest = &rest[1..];
        }
    }

    Some(rest.to_string())
}

fn ensure_parent_exists(path_str: &str) -> Result<()> {
    if path_str.eq_ignore_ascii_case(":memory:") || path_str.eq_ignore_ascii_case("memory") {
        return Ok(());
    }
    let p = PathBuf::from(path_str);
    if let Some(parent) = p.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create parent dir: {}", parent.display()))?;
        }
    }
    Ok(())
}

async fn run_migrations(pool: &Db) -> Result<()> {
    use sqlx::migrate::Migrator;
    let candidates = [
        Path::new("src-tauri/src/db/migrations"),
        Path::new("src/db/migrations"),
        Path::new("../src/db/migrations"),
        Path::new("../src-tauri/src/db/migrations"),
    ];
    for cand in candidates {
        if cand.exists() {
            let migrator = Migrator::new(cand).await?;
            migrator.run(pool).await?;
            return Ok(());
        }
    }
    Ok(())
}

pub async fn connect(database_url: &str) -> Result<Db> {
    if let Some(path_str) = extract_path(database_url) {
        ensure_parent_exists(&path_str)?;
        let opts = SqliteConnectOptions::from_str(&format!("sqlite://{}", path_str))
            .or_else(|_| SqliteConnectOptions::from_str(&format!("sqlite:///{}", path_str)))
            .unwrap_or_else(|_| SqliteConnectOptions::new().filename(Path::new(&path_str)))
            .create_if_missing(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(opts)
            .await?;
        run_migrations(&pool).await?;
        return Ok(pool);
    }

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await?;
    run_migrations(&pool).await?;
    Ok(pool)
}
