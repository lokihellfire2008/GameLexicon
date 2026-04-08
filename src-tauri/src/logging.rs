use chrono::Local;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

static LOG_WRITE_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

const APP_DIR_NAME: &str = "GameLexicon";
const LOG_DIR_NAME: &str = "logs";
const ENRICHMENT_LOG_FILE: &str = "enrichment.log";
const BACKUP_LOG_FILE: &str = "backup.log";
const DB_TRANSACTION_LOG_FILE: &str = "db-transactions.log";

pub fn logs_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(APP_DIR_NAME)
        .join(LOG_DIR_NAME)
}

pub fn log_enrichment(message: impl AsRef<str>) {
    append_log(ENRICHMENT_LOG_FILE, message.as_ref());
}

pub fn log_backup(message: impl AsRef<str>) {
    append_log(BACKUP_LOG_FILE, message.as_ref());
}

pub fn log_db_transaction(message: impl AsRef<str>) {
    append_log(DB_TRANSACTION_LOG_FILE, message.as_ref());
}

fn append_log(file_name: &str, message: &str) {
    let _guard = LOG_WRITE_LOCK.lock();
    let log_dir = logs_dir();

    if let Err(err) = fs::create_dir_all(&log_dir) {
        eprintln!(
            "[logging] Failed to create log directory '{}': {}",
            log_dir.display(),
            err
        );
        return;
    }

    let path = log_dir.join(file_name);
    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S%:z");
    let sanitized_message = sanitize_for_log(message);

    match OpenOptions::new().create(true).append(true).open(&path) {
        Ok(mut file) => {
            if let Err(err) = writeln!(file, "{} {}", timestamp, sanitized_message) {
                eprintln!("[logging] Failed to write '{}': {}", path.display(), err);
            }
        }
        Err(err) => {
            eprintln!("[logging] Failed to open '{}': {}", path.display(), err);
        }
    }
}

fn sanitize_for_log(message: &str) -> String {
    let collapsed = message
        .replace('\r', " ")
        .replace('\n', " | ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    if collapsed.is_empty() {
        "<empty>".to_string()
    } else {
        collapsed
    }
}
