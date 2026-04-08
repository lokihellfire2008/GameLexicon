#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod connectors;
mod db;
mod logging;
mod security;
mod services;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[tokio::main]
async fn main() {
    env_logger::init();

    // Default to a persistent file under LOCALAPPDATA\GameLexicon\gamelexicon.sqlite
    let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
        let base = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("GameLexicon");
        let _ = fs::create_dir_all(&base); // ignore errors; connect will fail loudly if needed
        let db_path = base.join("gamelexicon.sqlite");
        format!("sqlite://{}", db_path.to_string_lossy())
    });

    println!(">>> Using DATABASE_URL = {}", database_url);
    println!(
        ">>> CWD = {}",
        std::env::current_dir().unwrap_or_default().display()
    );
    let log_dir = logging::logs_dir();
    let _ = fs::create_dir_all(&log_dir);
    println!(">>> Log dir = {}", log_dir.display());

    let path_str = database_url
        .strip_prefix("sqlite:///")
        .or_else(|| database_url.strip_prefix("sqlite://"))
        .or_else(|| database_url.strip_prefix("sqlite:"))
        .unwrap_or(&database_url);

    if !path_str.eq_ignore_ascii_case(":memory:") && !path_str.eq_ignore_ascii_case("memory") {
        let p = PathBuf::from(path_str);
        if let Some(parent) = p.parent() {
            println!(">>> DB parent dir = {}", parent.display());
            println!(">>> DB parent exists = {}", parent.exists());
        } else {
            println!(">>> DB parent dir = <none>");
        }
        println!(
            ">>> Path looks absolute = {}",
            Path::new(path_str).is_absolute()
        );
    }

    let db_pool = match db::connect(&database_url).await {
        Ok(pool) => pool,
        Err(err) => {
            eprintln!("DB connection failed: {}", err);
            std::process::exit(1);
        }
    };

    tauri::Builder::default()
        // Native HTTP plugin (Tauri v2) so the frontend can call IGDB without CORS
        .plugin(tauri_plugin_http::init())
        // Shell plugin you already had
        .plugin(tauri_plugin_shell::init())
        .manage(db_pool)
        .invoke_handler(tauri::generate_handler![
            commands::add_manual_game,
            commands::list_games,
            commands::get_igdb_genres,
            commands::import_steam_games,
            commands::sync_steam_playtime,
            commands::save_steam_credentials,
            commands::get_steam_credentials,
            commands::save_twitch_credentials,
            commands::get_twitch_credentials,
            commands::update_game,
            commands::add_game_to_queue,
            commands::remove_game_from_queue,
            commands::reorder_queue,
            commands::add_queue_playtime,
            commands::export_library_backup,
            commands::import_library_backup,
            commands::export_queue_backup,
            commands::import_queue_backup,
            commands::default_backup_export_path,
            commands::save_backup_file,
            commands::open_external_link,
            commands::get_customization_settings,
            commands::save_customization_settings,
            commands::upload_customization_background,
            commands::load_customization_background_image,
            commands::enrich_genres_with_igdb,
            commands::get_game_details,
            commands::save_game_notes,
            commands::save_game_review,
            commands::lookup_igdb_preview,
            commands::apply_igdb_details,
            commands::lookup_igdb_candidates,
            commands::lookup_igdb_preview_by_id,
            commands::lookup_hltb_preview,
            commands::lookup_hltb_preview_by_id,
            commands::apply_hltb_details,
            commands::delete_game,
            commands::hide_game,
            commands::unhide_game,
            commands::load_amazon_rows_from_sqlite,
            commands::import_gog_owned_from_sqlite,
            commands::sync_gog_playtime_from_sqlite,
            commands::fetch_epic_owned_via_legendary,
            commands::xbox_begin_device_code,
            commands::xbox_finish_device_code_and_import_owned
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
