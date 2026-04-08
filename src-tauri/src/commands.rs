use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    QueryBuilder, Row, SqliteConnection,
};
use std::path::{Path, PathBuf};
use std::process::Command;
use tauri::Emitter;

use crate::{db::Db, logging};

fn normalize_license_type(input: Option<&str>) -> Result<&'static str, String> {
    let raw = input.map(|s| s.trim().to_ascii_lowercase());
    match raw.as_deref() {
        None | Some("") | Some("owned") => Ok("owned"),
        Some("subscription") => Ok("subscription"),
        Some("free") => Ok("free"),
        Some("trial") => Ok("trial"),
        Some(other) => Err(format!(
            "Invalid license_type '{}'. Expected one of: owned, subscription, free, trial.",
            other
        )),
    }
}

fn normalize_optional_text(input: Option<&str>) -> Option<String> {
    input
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn normalize_license_source(license_type: &str, source: Option<&str>) -> Option<String> {
    if license_type == "subscription" {
        normalize_optional_text(source)
    } else {
        None
    }
}

fn today_local_midnight_iso() -> String {
    let today = chrono::Local::now().date_naive();
    format!("{}T00:00:00Z", today.format("%Y-%m-%d"))
}

const APP_DATA_DIR_NAME: &str = "GameLexicon";
const CUSTOMIZATION_SETTINGS_FILE: &str = "customization.json";
const BACKGROUNDS_DIR_NAME: &str = "backgrounds";
const DEFAULT_BUTTON_COLOR: &str = "#0f766e";
const DEFAULT_TEXT_COLOR: &str = "#132033";
const DEFAULT_CARD_BACKGROUND_COLOR: &str = "#ffffff";
const DEFAULT_SECTION_BACKGROUND_COLOR: &str = "#f4f7fa";
const DEFAULT_PLATFORM_OPTIONS: &[&str] = &[
    "Steam",
    "Epic",
    "GOG",
    "EA",
    "Ubisoft",
    "Amazon",
    "Xbox",
    "Nintendo",
    "PlayStation",
    "Manual",
];
const DEFAULT_GENRE_OPTIONS: &[&str] = &[
    "Action",
    "Adventure",
    "RPG",
    "Shooter",
    "Strategy",
    "Puzzle",
    "Racing",
    "Sports",
    "Simulation",
    "Platform",
    "Hack and slash/Beat 'em up",
    "Indie",
    "Arcade",
    "Visual Novel",
    "Tactical",
    "RTS",
    "Turn-based strategy",
    "MOBA",
    "Fighting",
    "Music",
    "Rhythm",
    "Quiz/Trivia",
    "Pinball",
    "Roguelike",
    "Sandbox",
    "Survival",
    "Stealth",
    "Point-and-click",
    "Card & Board Game",
    "Battle Royale",
    "Tactical RPG",
    "JRPG",
    "Western RPG",
    "Metroidvania",
    "Party",
    "City Builder",
];

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CustomizationSettingsFile {
    button_color: String,
    text_color: String,
    card_background_color: String,
    section_background_color: String,
    background_image_name: Option<String>,
    #[serde(default = "default_platform_options")]
    platform_options: Vec<String>,
    #[serde(default = "default_genre_options")]
    genre_options: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomizationSettingsDto {
    pub button_color: String,
    pub text_color: String,
    pub card_background_color: String,
    pub section_background_color: String,
    pub background_image_name: Option<String>,
    pub background_image_data_url: Option<String>,
    pub available_backgrounds: Vec<String>,
    pub platform_options: Vec<String>,
    pub genre_options: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveCustomizationSettingsPayload {
    pub button_color: String,
    pub text_color: String,
    pub card_background_color: String,
    pub section_background_color: String,
    pub background_image_name: Option<String>,
    #[serde(default)]
    pub platform_options: Vec<String>,
    #[serde(default)]
    pub genre_options: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadCustomizationBackgroundPayload {
    pub file_name: String,
    pub mime_type: String,
    pub data_base64: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadCustomizationBackgroundResult {
    pub file_name: String,
    pub background_image_data_url: String,
    pub available_backgrounds: Vec<String>,
}

fn app_data_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(APP_DATA_DIR_NAME)
}

fn customization_settings_path() -> PathBuf {
    app_data_dir().join(CUSTOMIZATION_SETTINGS_FILE)
}

fn backgrounds_dir() -> PathBuf {
    app_data_dir().join(BACKGROUNDS_DIR_NAME)
}

fn default_platform_options() -> Vec<String> {
    DEFAULT_PLATFORM_OPTIONS
        .iter()
        .map(|value| (*value).to_string())
        .collect()
}

fn default_genre_options() -> Vec<String> {
    DEFAULT_GENRE_OPTIONS
        .iter()
        .map(|value| (*value).to_string())
        .collect()
}

fn normalize_option_list(values: &[String], defaults: &[&str]) -> Vec<String> {
    let mut cleaned: Vec<String> = Vec::new();

    for value in values {
        let Some(normalized) = normalize_optional_text(Some(value)) else {
            continue;
        };
        if cleaned
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(&normalized))
        {
            continue;
        }
        cleaned.push(normalized);
    }

    if cleaned.is_empty() {
        cleaned.extend(defaults.iter().map(|value| (*value).to_string()));
    }

    cleaned.sort_by(|a, b| a.to_ascii_lowercase().cmp(&b.to_ascii_lowercase()));
    cleaned
}

fn default_customization_settings() -> CustomizationSettingsFile {
    CustomizationSettingsFile {
        button_color: DEFAULT_BUTTON_COLOR.to_string(),
        text_color: DEFAULT_TEXT_COLOR.to_string(),
        card_background_color: DEFAULT_CARD_BACKGROUND_COLOR.to_string(),
        section_background_color: DEFAULT_SECTION_BACKGROUND_COLOR.to_string(),
        background_image_name: None,
        platform_options: default_platform_options(),
        genre_options: default_genre_options(),
    }
}

fn normalize_hex_color(raw: &str, fallback: &str) -> String {
    let trimmed = raw.trim();
    let valid = trimmed.len() == 7
        && trimmed.starts_with('#')
        && trimmed.chars().skip(1).all(|ch| ch.is_ascii_hexdigit());

    if valid {
        trimmed.to_ascii_lowercase()
    } else {
        fallback.to_string()
    }
}

fn read_customization_settings_file() -> CustomizationSettingsFile {
    let path = customization_settings_path();
    let Some(contents) = std::fs::read_to_string(&path).ok() else {
        return default_customization_settings();
    };
    let Ok(mut settings) = serde_json::from_str::<CustomizationSettingsFile>(&contents) else {
        return default_customization_settings();
    };

    settings.button_color = normalize_hex_color(&settings.button_color, DEFAULT_BUTTON_COLOR);
    settings.text_color = normalize_hex_color(&settings.text_color, DEFAULT_TEXT_COLOR);
    settings.card_background_color = normalize_hex_color(
        &settings.card_background_color,
        DEFAULT_CARD_BACKGROUND_COLOR,
    );
    settings.section_background_color = normalize_hex_color(
        &settings.section_background_color,
        DEFAULT_SECTION_BACKGROUND_COLOR,
    );
    settings.background_image_name = settings
        .background_image_name
        .as_deref()
        .and_then(|value| normalize_optional_text(Some(value)));
    settings.platform_options =
        normalize_option_list(&settings.platform_options, DEFAULT_PLATFORM_OPTIONS);
    settings.genre_options = normalize_option_list(&settings.genre_options, DEFAULT_GENRE_OPTIONS);

    settings
}

fn available_background_names() -> Vec<String> {
    let mut names = Vec::new();
    let dir = backgrounds_dir();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return names;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if image_mime_from_path(&path).is_none() {
            continue;
        }
        names.push(name.to_string());
    }

    names.sort_by_key(|name| name.to_ascii_lowercase());
    names
}

fn image_mime_from_extension(ext: &str) -> Option<&'static str> {
    match ext.trim_start_matches('.').to_ascii_lowercase().as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "webp" => Some("image/webp"),
        "gif" => Some("image/gif"),
        "bmp" => Some("image/bmp"),
        "svg" => Some("image/svg+xml"),
        _ => None,
    }
}

fn image_mime_from_path(path: &Path) -> Option<&'static str> {
    path.extension()
        .and_then(|ext| ext.to_str())
        .and_then(image_mime_from_extension)
}

fn infer_image_extension(file_name: &str, mime_type: &str) -> Option<&'static str> {
    if let Some(ext) = Path::new(file_name)
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
    {
        return match ext.as_str() {
            "png" => Some("png"),
            "jpg" | "jpeg" => Some("jpg"),
            "webp" => Some("webp"),
            "gif" => Some("gif"),
            "bmp" => Some("bmp"),
            "svg" => Some("svg"),
            _ => None,
        };
    }

    match mime_type.trim().to_ascii_lowercase().as_str() {
        "image/png" => Some("png"),
        "image/jpeg" | "image/jpg" => Some("jpg"),
        "image/webp" => Some("webp"),
        "image/gif" => Some("gif"),
        "image/bmp" => Some("bmp"),
        "image/svg+xml" => Some("svg"),
        _ => None,
    }
}

fn sanitize_background_stem(file_name: &str) -> String {
    let raw = Path::new(file_name)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("background");

    let sanitized = raw
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();

    let collapsed = sanitized
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");

    if collapsed.is_empty() {
        "background".to_string()
    } else {
        collapsed
    }
}

fn background_path_from_name(file_name: &str) -> Option<PathBuf> {
    let trimmed = file_name.trim();
    if trimmed.is_empty() {
        return None;
    }

    let path = backgrounds_dir().join(trimmed);
    let file_name_matches = path
        .file_name()
        .and_then(|value| value.to_str())
        .map(|value| value == trimmed)
        .unwrap_or(false);

    if file_name_matches {
        Some(path)
    } else {
        None
    }
}

fn read_background_data_url(file_name: &str) -> Result<Option<String>, String> {
    let Some(path) = background_path_from_name(file_name) else {
        return Ok(None);
    };
    if !path.exists() {
        return Ok(None);
    }

    let Some(mime_type) = image_mime_from_path(&path) else {
        return Ok(None);
    };
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    Ok(Some(format!(
        "data:{};base64,{}",
        mime_type,
        BASE64_STANDARD.encode(bytes)
    )))
}

fn customization_settings_to_dto(
    settings: CustomizationSettingsFile,
) -> Result<CustomizationSettingsDto, String> {
    let available_backgrounds = available_background_names();
    let background_image_name = settings.background_image_name.and_then(|name| {
        if available_backgrounds
            .iter()
            .any(|candidate| candidate == &name)
        {
            Some(name)
        } else {
            None
        }
    });
    let background_image_data_url = match background_image_name.as_deref() {
        Some(name) => read_background_data_url(name)?,
        None => None,
    };

    Ok(CustomizationSettingsDto {
        button_color: settings.button_color,
        text_color: settings.text_color,
        card_background_color: settings.card_background_color,
        section_background_color: settings.section_background_color,
        background_image_name,
        background_image_data_url,
        available_backgrounds,
        platform_options: settings.platform_options,
        genre_options: settings.genre_options,
    })
}

fn persist_customization_settings(settings: &CustomizationSettingsFile) -> Result<(), String> {
    let app_dir = app_data_dir();
    std::fs::create_dir_all(&app_dir).map_err(|e| e.to_string())?;
    let payload = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    std::fs::write(customization_settings_path(), payload).map_err(|e| e.to_string())
}

#[derive(Clone)]
struct GameAuditRecord {
    title: String,
    platform: String,
}

async fn fetch_game_audit_record<'e, E>(
    executor: E,
    id: &str,
) -> Result<Option<GameAuditRecord>, sqlx::Error>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let row = sqlx::query("SELECT canonical_title, platform FROM games WHERE id = ?")
        .bind(id)
        .fetch_optional(executor)
        .await?;

    Ok(row.map(|row| GameAuditRecord {
        title: row.get(0),
        platform: row.get(1),
    }))
}

fn normalize_for_log(raw: &str) -> String {
    raw.replace('\r', " ")
        .replace('\n', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn shorten_for_log(raw: &str, max_chars: usize) -> String {
    let normalized = normalize_for_log(raw);
    let char_count = normalized.chars().count();
    if char_count <= max_chars {
        return normalized;
    }

    let truncated: String = normalized
        .chars()
        .take(max_chars.saturating_sub(3))
        .collect();
    format!("{}...", truncated)
}

fn quote_for_log(raw: &str, max_chars: usize) -> String {
    format!("{:?}", shorten_for_log(raw, max_chars))
}

fn format_optional_text_for_log(value: Option<&str>, max_chars: usize) -> String {
    value
        .map(|text| quote_for_log(text, max_chars))
        .unwrap_or_else(|| "null".to_string())
}

fn format_game_identity(title: &str, platform: &str, id: &str) -> String {
    format!(
        "title={} platform={} id={}",
        quote_for_log(title, 96),
        quote_for_log(platform, 48),
        id
    )
}

fn format_game_audit_record(game: Option<&GameAuditRecord>, id: &str) -> String {
    match game {
        Some(game) => format_game_identity(&game.title, &game.platform, id),
        None => format!("id={}", id),
    }
}

fn join_log_parts(parts: Vec<String>) -> String {
    if parts.is_empty() {
        "none".to_string()
    } else {
        parts.join(", ")
    }
}

fn summarize_game_update(changes: &GameUpdate) -> Vec<String> {
    let mut fields = Vec::new();

    if let Some(value) = changes.canonical_title.as_deref() {
        fields.push(format!("title={}", quote_for_log(value, 96)));
    }
    if let Some(value) = changes.platform.as_deref() {
        fields.push(format!("platform={}", quote_for_log(value, 48)));
    }
    if let Some(value) = changes.beaten {
        fields.push(format!("beaten={}", value));
    }
    if let Some(value) = changes.genre.as_deref() {
        fields.push(format!("genre={}", quote_for_log(value, 96)));
    }
    if let Some(value) = changes.playtime_minutes {
        fields.push(format!("playtime_minutes={}", value));
    }
    if let Some(value) = changes.last_played_at.as_deref() {
        let normalized = normalize_sqlite_datetime_to_iso(Some(value));
        fields.push(format!(
            "last_played_at={}",
            format_optional_text_for_log(normalized.as_deref(), 32)
        ));
    }
    if let Some(value) = changes.queue_is_playing {
        fields.push(format!("queue_is_playing={}", value));
    }
    if let Some(value) = changes.queue_started_at.as_deref() {
        let normalized = normalize_sqlite_datetime_to_iso(Some(value));
        fields.push(format!(
            "queue_started_at={}",
            format_optional_text_for_log(normalized.as_deref(), 32)
        ));
    }
    if let Some(value) = changes.queue_finished_at.as_deref() {
        let normalized = normalize_sqlite_datetime_to_iso(Some(value));
        fields.push(format!(
            "queue_finished_at={}",
            format_optional_text_for_log(normalized.as_deref(), 32)
        ));
    }
    if let Some(value) = changes.cover_url.as_deref() {
        fields.push(format!("cover_url={}", quote_for_log(value, 140)));
    }
    if let Some(value) = changes.release_year {
        fields.push(format!("release_year={}", value));
    }
    if let Some(value) = changes.license_type.as_deref() {
        let normalized = normalize_license_type(Some(value)).unwrap_or(value);
        fields.push(format!("license_type={}", quote_for_log(normalized, 24)));
    }
    if let Some(value) = changes.license_source.as_deref() {
        let normalized = normalize_optional_text(Some(value));
        fields.push(format!(
            "license_source={}",
            format_optional_text_for_log(normalized.as_deref(), 64)
        ));
    }

    fields
}

fn summarize_igdb_apply_payload(payload: &IgdbApplyPayload) -> Vec<String> {
    let mut fields = Vec::new();

    if let Some(value) = payload.igdb_id {
        fields.push(format!("igdb_id={}", value));
    }
    if let Some(value) = payload
        .igdb_url
        .as_deref()
        .and_then(|url| normalize_optional_text(Some(url)))
    {
        fields.push(format!("igdb_url={}", quote_for_log(&value, 140)));
    }
    if let Some(value) = payload.genres.as_ref().filter(|genres| !genres.is_empty()) {
        fields.push(format!("genres={}", quote_for_log(&value.join(", "), 120)));
    }
    if let Some(value) = payload.cover_url.as_deref().filter(|url| !url.is_empty()) {
        fields.push(format!("cover_url={}", quote_for_log(value, 140)));
    }
    if let Some(value) = payload.release_year {
        fields.push(format!("release_year={}", value));
    }
    if let Some(value) = payload
        .summary
        .as_deref()
        .and_then(|text| normalize_optional_text(Some(text)))
    {
        fields.push(format!("summary_chars={}", value.chars().count()));
    }
    if let Some(value) = payload
        .storyline
        .as_deref()
        .and_then(|text| normalize_optional_text(Some(text)))
    {
        fields.push(format!("storyline_chars={}", value.chars().count()));
    }
    if let Some(value) = payload.agg_rating {
        fields.push(format!("agg_rating={:.2}", value));
    }
    if let Some(value) = payload.user_rating {
        fields.push(format!("user_rating={:.2}", value));
    }
    if let Some(value) = payload.ttb_main {
        fields.push(format!("ttb_main={}", value));
    }
    if let Some(value) = payload.ttb_extra {
        fields.push(format!("ttb_extra={}", value));
    }
    if let Some(value) = payload.ttb_complete {
        fields.push(format!("ttb_complete={}", value));
    }
    if let Some(value) = payload.ttb_count {
        fields.push(format!("ttb_count={}", value));
    }

    fields
}

fn summarize_hltb_apply_payload(payload: &HltbApplyPayload) -> Vec<String> {
    let mut fields = Vec::new();

    if let Some(value) = payload.hltb_id {
        fields.push(format!("hltb_id={}", value));
    }
    if let Some(value) = payload
        .title
        .as_deref()
        .and_then(|text| normalize_optional_text(Some(text)))
    {
        fields.push(format!("hltb_title={}", quote_for_log(&value, 96)));
    }
    if let Some(value) = payload
        .profile_url
        .as_deref()
        .and_then(|url| normalize_optional_text(Some(url)))
    {
        fields.push(format!("hltb_url={}", quote_for_log(&value, 140)));
    }
    if let Some(value) = payload.hltb_main {
        fields.push(format!("hltb_main={}", value));
    }
    if let Some(value) = payload.hltb_extra {
        fields.push(format!("hltb_extra={}", value));
    }
    if let Some(value) = payload.hltb_complete {
        fields.push(format!("hltb_complete={}", value));
    }
    if let Some(value) = payload.hltb_all {
        fields.push(format!("hltb_all={}", value));
    }
    if let Some(value) = payload.hltb_main_count {
        fields.push(format!("hltb_main_count={}", value));
    }
    if let Some(value) = payload.hltb_extra_count {
        fields.push(format!("hltb_extra_count={}", value));
    }
    if let Some(value) = payload.hltb_complete_count {
        fields.push(format!("hltb_complete_count={}", value));
    }
    if let Some(value) = payload.hltb_all_count {
        fields.push(format!("hltb_all_count={}", value));
    }
    if let Some(value) = payload
        .platforms
        .as_deref()
        .and_then(|text| normalize_optional_text(Some(text)))
    {
        fields.push(format!("platforms={}", quote_for_log(&value, 120)));
    }
    if let Some(value) = payload
        .genres
        .as_deref()
        .and_then(|text| normalize_optional_text(Some(text)))
    {
        fields.push(format!("genres={}", quote_for_log(&value, 120)));
    }
    if let Some(value) = payload
        .summary
        .as_deref()
        .and_then(|text| normalize_optional_text(Some(text)))
    {
        fields.push(format!("summary_chars={}", value.chars().count()));
    }
    if let Some(value) = payload
        .match_method
        .as_deref()
        .and_then(|text| normalize_optional_text(Some(text)))
    {
        fields.push(format!("match_method={}", quote_for_log(&value, 48)));
    }

    fields
}

fn normalize_external_http_url(url: &str) -> Result<String, String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Err("URL is required.".to_string());
    }

    let parsed = reqwest::Url::parse(trimmed).map_err(|_| "Invalid URL.".to_string())?;
    match parsed.scheme() {
        "http" | "https" => Ok(parsed.into()),
        _ => Err("Only http:// and https:// links can be opened.".to_string()),
    }
}

#[tauri::command]
pub async fn open_external_link(url: String) -> Result<(), String> {
    let normalized = normalize_external_http_url(&url)?;

    #[cfg(target_os = "windows")]
    let mut command = {
        let mut cmd = Command::new("cmd");
        cmd.arg("/C").arg("start").arg("").arg(&normalized);
        cmd
    };

    #[cfg(target_os = "macos")]
    let mut command = {
        let mut cmd = Command::new("open");
        cmd.arg(&normalized);
        cmd
    };

    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut cmd = Command::new("xdg-open");
        cmd.arg(&normalized);
        cmd
    };

    command
        .spawn()
        .map_err(|e| format!("Could not open external link: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn get_customization_settings() -> Result<CustomizationSettingsDto, String> {
    customization_settings_to_dto(read_customization_settings_file())
}

#[tauri::command]
pub async fn save_customization_settings(
    payload: SaveCustomizationSettingsPayload,
) -> Result<CustomizationSettingsDto, String> {
    let available_backgrounds = available_background_names();
    let background_image_name = payload
        .background_image_name
        .as_deref()
        .and_then(|value| normalize_optional_text(Some(value)));

    if let Some(name) = background_image_name.as_deref() {
        if !available_backgrounds
            .iter()
            .any(|candidate| candidate == name)
        {
            return Err("The selected background image is no longer available.".to_string());
        }
    }

    let settings = CustomizationSettingsFile {
        button_color: normalize_hex_color(&payload.button_color, DEFAULT_BUTTON_COLOR),
        text_color: normalize_hex_color(&payload.text_color, DEFAULT_TEXT_COLOR),
        card_background_color: normalize_hex_color(
            &payload.card_background_color,
            DEFAULT_CARD_BACKGROUND_COLOR,
        ),
        section_background_color: normalize_hex_color(
            &payload.section_background_color,
            DEFAULT_SECTION_BACKGROUND_COLOR,
        ),
        background_image_name,
        platform_options: normalize_option_list(
            &payload.platform_options,
            DEFAULT_PLATFORM_OPTIONS,
        ),
        genre_options: normalize_option_list(&payload.genre_options, DEFAULT_GENRE_OPTIONS),
    };

    persist_customization_settings(&settings)?;
    logging::log_db_transaction(format!(
        "Customization settings updated button_color={} text_color={} card_background_color={} section_background_color={} background_image={} platform_options={} genre_options={}",
        settings.button_color,
        settings.text_color,
        settings.card_background_color,
        settings.section_background_color,
        settings
            .background_image_name
            .as_deref()
            .map(|value| quote_for_log(value, 120))
            .unwrap_or_else(|| "none".to_string()),
        settings.platform_options.len(),
        settings.genre_options.len()
    ));

    customization_settings_to_dto(settings)
}

#[tauri::command]
pub async fn upload_customization_background(
    payload: UploadCustomizationBackgroundPayload,
) -> Result<UploadCustomizationBackgroundResult, String> {
    let mime_type = payload.mime_type.trim().to_ascii_lowercase();
    if !mime_type.starts_with("image/") {
        return Err("Only image uploads are supported for backgrounds.".to_string());
    }

    let extension = infer_image_extension(&payload.file_name, &mime_type).ok_or_else(|| {
        "Unsupported image format. Please use PNG, JPG, WEBP, GIF, BMP, or SVG.".to_string()
    })?;

    let bytes = BASE64_STANDARD
        .decode(payload.data_base64.trim())
        .map_err(|_| "Could not decode the selected image.".to_string())?;

    if bytes.is_empty() {
        return Err("The selected image file was empty.".to_string());
    }
    if bytes.len() > 15 * 1024 * 1024 {
        return Err("Please choose an image smaller than 15 MB.".to_string());
    }

    let dir = backgrounds_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let stem = sanitize_background_stem(&payload.file_name);
    let file_name = format!("{}-{}.{}", stem, uuid::Uuid::new_v4().simple(), extension);
    let path = dir.join(&file_name);

    std::fs::write(&path, &bytes).map_err(|e| e.to_string())?;

    logging::log_db_transaction(format!(
        "Customization background uploaded file_name={} bytes={}",
        quote_for_log(&file_name, 160),
        bytes.len()
    ));

    Ok(UploadCustomizationBackgroundResult {
        file_name,
        background_image_data_url: format!(
            "data:{};base64,{}",
            mime_type,
            BASE64_STANDARD.encode(bytes)
        ),
        available_backgrounds: available_background_names(),
    })
}

#[tauri::command]
pub async fn load_customization_background_image(
    file_name: String,
) -> Result<Option<String>, String> {
    read_background_data_url(&file_name)
}

#[derive(Serialize)]
pub struct AmazonSqlitePreviewRow {
    pub title: String,
    pub release_year: Option<i64>,
    pub genres: Option<Vec<String>>,
    pub asin: Option<String>,
    pub sku: Option<String>,
    pub owned: Option<bool>,
}

fn quote_ident(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\"\""))
}

fn pick_first_column(columns: &[String], candidates: &[&str]) -> Option<String> {
    for cand in candidates {
        if let Some(hit) = columns.iter().find(|c| c.eq_ignore_ascii_case(cand)) {
            return Some(hit.clone());
        }
    }
    None
}

fn parse_year_from_text(raw: Option<&str>) -> Option<i64> {
    let s = raw?.trim();
    if s.len() < 4 {
        return None;
    }
    let bytes = s.as_bytes();
    for i in 0..=bytes.len().saturating_sub(4) {
        let y = &bytes[i..i + 4];
        if y.iter().all(u8::is_ascii_digit) {
            if let Ok(v) = std::str::from_utf8(y).ok()?.parse::<i64>() {
                if (1970..=2200).contains(&v) {
                    return Some(v);
                }
            }
        }
    }
    None
}

fn parse_owned_flag(raw: Option<&str>) -> Option<bool> {
    let s = raw?.trim().to_ascii_lowercase();
    match s.as_str() {
        "1" | "true" | "yes" | "owned" => Some(true),
        "0" | "false" | "no" => Some(false),
        _ => None,
    }
}

fn parse_genres_field(raw: Option<&str>) -> Option<Vec<String>> {
    let s = raw?.trim();
    if s.is_empty() {
        return None;
    }

    let mut out: Vec<String> = if s.starts_with('[') {
        match serde_json::from_str::<serde_json::Value>(s) {
            Ok(serde_json::Value::Array(arr)) => arr
                .into_iter()
                .filter_map(|v| {
                    v.as_str()
                        .map(str::to_string)
                        .or_else(|| v.get("name").and_then(|n| n.as_str()).map(str::to_string))
                })
                .collect(),
            _ => Vec::new(),
        }
    } else {
        s.split(',').map(str::to_string).collect()
    };

    out = out
        .into_iter()
        .map(|g| g.trim().to_string())
        .filter(|g| !g.is_empty())
        .collect();
    out.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
    out.dedup_by(|a, b| a.eq_ignore_ascii_case(b));

    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

fn normalize_title_for_dedupe(s: &str) -> String {
    s.to_ascii_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn push_sqlite_candidate(
    candidates: &mut Vec<PathBuf>,
    seen: &mut std::collections::HashSet<String>,
    p: PathBuf,
) {
    if !p.exists() || !p.is_file() {
        return;
    }
    let key = p.to_string_lossy().to_ascii_lowercase();
    if seen.insert(key) {
        candidates.push(p);
    }
}

fn scan_sqlite_files(
    dir: &Path,
    candidates: &mut Vec<PathBuf>,
    seen: &mut std::collections::HashSet<String>,
) {
    if !dir.exists() || !dir.is_dir() {
        return;
    }
    if let Ok(rd) = std::fs::read_dir(dir) {
        for entry in rd.flatten() {
            let p = entry.path();
            if p.is_file() {
                if let Some(ext) = p.extension().and_then(|x| x.to_str()) {
                    let ext = ext.to_ascii_lowercase();
                    if ext == "db" || ext == "sqlite" || ext == "sqlite3" {
                        push_sqlite_candidate(candidates, seen, p);
                    }
                }
            }
        }
    }
}

fn collect_amazon_sqlite_candidates(override_path: Option<&str>) -> Result<Vec<PathBuf>, String> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    let mut seen = std::collections::HashSet::<String>::new();

    if let Some(p) = override_path.map(str::trim).filter(|s| !s.is_empty()) {
        let selected = PathBuf::from(p);
        if !selected.exists() {
            return Err(format!("Amazon SQLite file not found at: {}", p));
        }
        push_sqlite_candidate(&mut candidates, &mut seen, selected.clone());

        // If the user picked a key/value DB (e.g., Entitlements.sqlite), probe sibling DBs too.
        if let Some(parent) = selected.parent() {
            for name in [
                "GameProductInfo.sqlite",
                "GameInstallInfo.sqlite",
                "GameInstallMetadataInfo.sqlite",
                "GameUserInteractionsInfo.sqlite",
                "ProductDetails.sqlite",
                "Entitlements.sqlite",
            ] {
                push_sqlite_candidate(&mut candidates, &mut seen, parent.join(name));
            }
            scan_sqlite_files(parent, &mut candidates, &mut seen);
        }
        return Ok(candidates);
    }

    let local = std::env::var("LOCALAPPDATA")
        .map(PathBuf::from)
        .map_err(|_| {
            "LOCALAPPDATA is not available; provide an Amazon DB path manually.".to_string()
        })?;

    let data_dir = local.join("Amazon Games").join("Data");
    let sql_dir = data_dir.join("Games").join("Sql");

    for p in [
        sql_dir.join("GameProductInfo.sqlite"),
        sql_dir.join("GameInstallInfo.sqlite"),
        sql_dir.join("GameInstallMetadataInfo.sqlite"),
        sql_dir.join("GameUserInteractionsInfo.sqlite"),
        sql_dir.join("ProductDetails.sqlite"),
        sql_dir.join("Entitlements.sqlite"),
        data_dir.join("Games.db"),
        data_dir.join("Games.sqlite"),
        data_dir.join("GameLibrary.sqlite"),
        data_dir.join("Sql").join("Games.db"),
        data_dir.join("Sql").join("Games.sqlite"),
        data_dir.join("Sql").join("GameLibrary.sqlite"),
    ] {
        push_sqlite_candidate(&mut candidates, &mut seen, p);
    }

    scan_sqlite_files(&sql_dir, &mut candidates, &mut seen);
    scan_sqlite_files(&data_dir, &mut candidates, &mut seen);
    if let Ok(rd) = std::fs::read_dir(&data_dir) {
        for entry in rd.flatten() {
            let p = entry.path();
            if p.is_dir() {
                scan_sqlite_files(&p, &mut candidates, &mut seen);
            }
        }
    }

    if candidates.is_empty() {
        return Err(format!(
            "Could not auto-detect an Amazon Games SQLite DB under {}. Paste a DB path manually.",
            data_dir.display()
        ));
    }
    Ok(candidates)
}

fn parse_gog_title(raw: Option<&str>) -> Option<String> {
    let s = normalize_optional_text(raw)?;
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(&s) {
        if let Some(t) = v.get("title").and_then(|x| x.as_str()) {
            return normalize_optional_text(Some(t));
        }
        if let Some(t) = v.get("name").and_then(|x| x.as_str()) {
            return normalize_optional_text(Some(t));
        }
        if let Some(t) = v.as_str() {
            return normalize_optional_text(Some(t));
        }
    }
    Some(s)
}

fn parse_gog_release_year(meta: &serde_json::Value) -> Option<i64> {
    use chrono::Datelike;

    let Some(raw) = meta.get("releaseDate") else {
        return None;
    };

    if let Some(n) = raw.as_i64() {
        let mut secs = n;
        if secs > 10_000_000_000 {
            secs /= 1000;
        }
        if let Some(dt) = chrono::DateTime::<chrono::Utc>::from_timestamp(secs, 0) {
            let y = dt.year() as i64;
            if (1970..=2200).contains(&y) {
                return Some(y);
            }
        }
        return parse_year_from_text(Some(&n.to_string()));
    }

    parse_year_from_text(raw.as_str())
}

fn parse_gog_meta(raw: Option<&str>) -> (Option<i64>, Option<Vec<String>>) {
    let s = raw.unwrap_or("").trim();
    if s.is_empty() {
        return (None, None);
    }

    let mut release_year: Option<i64> = None;
    let mut genres: Vec<String> = Vec::new();

    if let Ok(v) = serde_json::from_str::<serde_json::Value>(s) {
        release_year = parse_gog_release_year(&v);

        if let Some(arr) = v.get("genres").and_then(|x| x.as_array()) {
            for g in arr {
                if let Some(name) = g.as_str() {
                    if let Some(clean) = normalize_optional_text(Some(name)) {
                        genres.push(clean);
                    }
                } else if let Some(name) = g.get("name").and_then(|x| x.as_str()) {
                    if let Some(clean) = normalize_optional_text(Some(name)) {
                        genres.push(clean);
                    }
                }
            }
        }
    }

    genres.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
    genres.dedup_by(|a, b| a.eq_ignore_ascii_case(b));

    let genre_out = if genres.is_empty() {
        None
    } else {
        Some(genres)
    };
    (release_year, genre_out)
}

fn normalize_sqlite_datetime_to_iso(raw: Option<&str>) -> Option<String> {
    let s = normalize_optional_text(raw)?;
    if s.contains('T') {
        return Some(s);
    }

    if let Ok(ndt) = chrono::NaiveDateTime::parse_from_str(&s, "%Y-%m-%d %H:%M:%S") {
        let dt = chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(ndt, chrono::Utc);
        return Some(dt.to_rfc3339());
    }

    if let Ok(d) = chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d") {
        return Some(format!("{}T00:00:00Z", d.format("%Y-%m-%d")));
    }

    Some(s)
}

fn strip_marketplace_suffixes(title: &str) -> (String, bool) {
    let trimmed = title.trim();
    if trimmed.is_empty() {
        return (String::new(), false);
    }

    // Known cross-store labels that can leak into Galaxy title text.
    const SUFFIXES: &[&str] = &[
        " - amazon prime",
        " - amazon luna",
        " - prime gaming",
        " - amazon",
        " (amazon prime)",
        " (amazon luna)",
        " (prime gaming)",
    ];

    let lower = trimmed.to_ascii_lowercase();
    for suf in SUFFIXES {
        if lower.ends_with(suf) {
            let base = trimmed
                .get(..trimmed.len().saturating_sub(suf.len()))
                .unwrap_or("")
                .trim()
                .trim_end_matches(&['-', '–', ':', '|', ' '][..])
                .trim()
                .to_string();
            if !base.is_empty() {
                return (base, true);
            }
        }
    }

    (trimmed.to_string(), false)
}

fn collect_gog_sqlite_candidates(override_path: Option<&str>) -> Result<Vec<PathBuf>, String> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    let mut seen = std::collections::HashSet::<String>::new();

    if let Some(p) = override_path.map(str::trim).filter(|s| !s.is_empty()) {
        let selected = PathBuf::from(p);
        if !selected.exists() {
            return Err(format!("GOG SQLite path not found: {}", p));
        }

        if selected.is_dir() {
            push_sqlite_candidate(&mut candidates, &mut seen, selected.join("galaxy-2.0.db"));
            scan_sqlite_files(&selected, &mut candidates, &mut seen);
        } else {
            push_sqlite_candidate(&mut candidates, &mut seen, selected.clone());
            if let Some(parent) = selected.parent() {
                push_sqlite_candidate(&mut candidates, &mut seen, parent.join("galaxy-2.0.db"));
                scan_sqlite_files(parent, &mut candidates, &mut seen);
            }
        }

        if candidates.is_empty() {
            return Err(format!("No SQLite files found at: {}", selected.display()));
        }
        return Ok(candidates);
    }

    let mut roots: Vec<PathBuf> = Vec::new();
    if let Ok(pd) = std::env::var("PROGRAMDATA") {
        roots.push(PathBuf::from(pd));
    }
    roots.push(PathBuf::from(r"C:\ProgramData"));

    let mut seen_roots = std::collections::HashSet::<String>::new();
    for root in roots {
        let root_key = root.to_string_lossy().to_ascii_lowercase();
        if !seen_roots.insert(root_key) {
            continue;
        }

        let storage = root.join("GOG.com").join("Galaxy").join("storage");
        push_sqlite_candidate(&mut candidates, &mut seen, storage.join("galaxy-2.0.db"));
        scan_sqlite_files(&storage, &mut candidates, &mut seen);
    }

    if candidates.is_empty() {
        return Err("Could not auto-detect GOG Galaxy DB. Provide a path manually.".to_string());
    }

    Ok(candidates)
}

#[tauri::command]
pub async fn load_amazon_rows_from_sqlite(
    db_path: Option<String>,
) -> Result<Vec<AmazonSqlitePreviewRow>, String> {
    const TITLE_COLS: &[&str] = &["ProductTitle", "Title", "GameTitle", "DisplayName", "Name"];
    const RELEASE_COLS: &[&str] = &["ReleaseDate", "Release Year", "ReleaseYear", "Year"];
    const GENRE_COLS: &[&str] = &["GenresJson", "Genres", "Genre"];
    const ASIN_COLS: &[&str] = &["ProductAsin", "ASIN", "Asin"];
    const SKU_COLS: &[&str] = &["ProductSku", "SkuId", "SKU", "Skuid"];
    const OWNED_COLS: &[&str] = &["IsOwned", "Owned", "Installed"];

    let db_files = collect_amazon_sqlite_candidates(db_path.as_deref())?;
    let mut tried: Vec<String> = Vec::new();

    for db_file in db_files {
        tried.push(db_file.display().to_string());
        if !Path::new(&db_file).exists() {
            continue;
        }

        let opts = SqliteConnectOptions::new()
            .filename(&db_file)
            .read_only(true);
        let pool = match SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await
        {
            Ok(p) => p,
            Err(_) => continue,
        };

        let table_rows = match sqlx::query(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
        )
        .fetch_all(&pool)
        .await
        {
            Ok(v) => v,
            Err(_) => continue,
        };

        let mut best: Option<(String, Vec<String>, i32)> = None;
        for tr in table_rows {
            let table_name: String = tr.get(0);
            let pragma = format!("PRAGMA table_info({})", quote_ident(&table_name));
            let info_rows = match sqlx::query(&pragma).fetch_all(&pool).await {
                Ok(v) => v,
                Err(_) => continue,
            };
            if info_rows.is_empty() {
                continue;
            }

            let mut columns: Vec<String> = Vec::new();
            for ir in info_rows {
                let col = ir
                    .try_get::<String, _>("name")
                    .or_else(|_| ir.try_get::<String, _>(1))
                    .unwrap_or_default();
                if !col.is_empty() {
                    columns.push(col);
                }
            }
            if columns.is_empty() {
                continue;
            }

            if pick_first_column(&columns, TITLE_COLS).is_none() {
                continue;
            }

            let mut score = 10;
            if pick_first_column(&columns, RELEASE_COLS).is_some() {
                score += 1;
            }
            if pick_first_column(&columns, GENRE_COLS).is_some() {
                score += 1;
            }
            if pick_first_column(&columns, ASIN_COLS).is_some() {
                score += 1;
            }
            if pick_first_column(&columns, SKU_COLS).is_some() {
                score += 1;
            }
            if pick_first_column(&columns, OWNED_COLS).is_some() {
                score += 1;
            }
            if table_name.eq_ignore_ascii_case("DbSet") {
                score += 3;
            }
            if table_name.to_ascii_lowercase().contains("game") {
                score += 1;
            }

            match &best {
                Some((_, _, best_score)) if *best_score >= score => {}
                _ => best = Some((table_name, columns, score)),
            }
        }

        let Some((table_name, columns, _)) = best else {
            continue;
        };

        let Some(title_col) = pick_first_column(&columns, TITLE_COLS) else {
            continue;
        };
        let release_col = pick_first_column(&columns, RELEASE_COLS);
        let genres_col = pick_first_column(&columns, GENRE_COLS);
        let asin_col = pick_first_column(&columns, ASIN_COLS);
        let sku_col = pick_first_column(&columns, SKU_COLS);
        let owned_col = pick_first_column(&columns, OWNED_COLS);

        let title_expr = format!("CAST({} AS TEXT)", quote_ident(&title_col));
        let release_expr = release_col
            .as_ref()
            .map(|c| format!("CAST({} AS TEXT)", quote_ident(c)))
            .unwrap_or_else(|| "NULL".to_string());
        let genres_expr = genres_col
            .as_ref()
            .map(|c| format!("CAST({} AS TEXT)", quote_ident(c)))
            .unwrap_or_else(|| "NULL".to_string());
        let asin_expr = asin_col
            .as_ref()
            .map(|c| format!("CAST({} AS TEXT)", quote_ident(c)))
            .unwrap_or_else(|| "NULL".to_string());
        let sku_expr = sku_col
            .as_ref()
            .map(|c| format!("CAST({} AS TEXT)", quote_ident(c)))
            .unwrap_or_else(|| "NULL".to_string());
        let owned_expr = owned_col
            .as_ref()
            .map(|c| format!("CAST({} AS TEXT)", quote_ident(c)))
            .unwrap_or_else(|| "NULL".to_string());

        let sql = format!(
            r#"
            SELECT
              {title_expr}   AS title_raw,
              {release_expr} AS release_raw,
              {genres_expr}  AS genres_raw,
              {asin_expr}    AS asin_raw,
              {sku_expr}     AS sku_raw,
              {owned_expr}   AS owned_raw
            FROM {table_name}
            WHERE TRIM(COALESCE({title_expr}, '')) <> ''
            "#,
            table_name = quote_ident(&table_name),
        );

        let rows = match sqlx::query(&sql).fetch_all(&pool).await {
            Ok(v) => v,
            Err(_) => continue,
        };

        let mut seen_titles = std::collections::HashSet::<String>::new();
        let mut out: Vec<AmazonSqlitePreviewRow> = Vec::new();
        for r in rows {
            let title = r
                .try_get::<Option<String>, _>("title_raw")
                .unwrap_or(None)
                .unwrap_or_default()
                .trim()
                .to_string();
            if title.is_empty() {
                continue;
            }

            let key = normalize_title_for_dedupe(&title);
            if key.is_empty() || !seen_titles.insert(key) {
                continue;
            }

            let release_raw = r
                .try_get::<Option<String>, _>("release_raw")
                .unwrap_or(None);
            let genres_raw = r.try_get::<Option<String>, _>("genres_raw").unwrap_or(None);
            let asin_raw = r.try_get::<Option<String>, _>("asin_raw").unwrap_or(None);
            let sku_raw = r.try_get::<Option<String>, _>("sku_raw").unwrap_or(None);
            let owned_raw = r.try_get::<Option<String>, _>("owned_raw").unwrap_or(None);

            out.push(AmazonSqlitePreviewRow {
                title,
                release_year: parse_year_from_text(release_raw.as_deref()),
                genres: parse_genres_field(genres_raw.as_deref()),
                asin: normalize_optional_text(asin_raw.as_deref()),
                sku: normalize_optional_text(sku_raw.as_deref()),
                owned: parse_owned_flag(owned_raw.as_deref()),
            });
        }

        if !out.is_empty() {
            out.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
            return Ok(out);
        }
    }

    Err(format!(
        "Could not read recognizable Amazon game rows from the selected location. Tried: {}",
        tried.join(", ")
    ))
}

#[derive(Clone)]
struct GogImportCandidate {
    release_key: String,
    title: String,
    playtime_minutes: Option<i64>,
    last_played_at: Option<String>,
    genre: Option<String>,
    release_year: Option<i64>,
    score: i32,
}

async fn load_gog_import_candidates(
    db_path: Option<&str>,
) -> Result<(PathBuf, Vec<GogImportCandidate>), String> {
    let db_files = collect_gog_sqlite_candidates(db_path)?;
    let mut tried: Vec<String> = Vec::new();

    for db_file in db_files {
        tried.push(db_file.display().to_string());
        if !Path::new(&db_file).exists() {
            continue;
        }

        let opts = SqliteConnectOptions::new()
            .filename(&db_file)
            .read_only(true);
        let source_pool = match SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(opts)
            .await
        {
            Ok(p) => p,
            Err(_) => continue,
        };

        let rows = match sqlx::query(
            r#"
            SELECT
              lr.releaseKey AS release_key,
              COALESCE(
                (
                  SELECT gp.value
                  FROM GamePieces gp
                  JOIN GamePieceTypes gpt ON gpt.id = gp.gamePieceTypeId
                  WHERE gp.releaseKey = lr.releaseKey AND gpt.type = 'title'
                  ORDER BY gp.userId DESC, gp.languageId ASC
                  LIMIT 1
                ),
                (
                  SELECT gp.value
                  FROM GamePieces gp
                  JOIN GamePieceTypes gpt ON gpt.id = gp.gamePieceTypeId
                  WHERE gp.releaseKey = lr.releaseKey AND gpt.type = 'originalTitle'
                  ORDER BY gp.userId DESC, gp.languageId ASC
                  LIMIT 1
                ),
                (
                  SELECT ld.title
                  FROM LimitedDetails ld
                  JOIN ProductsToReleaseKeys ptr2 ON ptr2.gogId = ld.productId
                  WHERE ptr2.releaseKey = lr.releaseKey
                  ORDER BY ld.stored_at DESC
                  LIMIT 1
                )
              ) AS title_raw,
              (
                SELECT gp.value
                FROM GamePieces gp
                JOIN GamePieceTypes gpt ON gpt.id = gp.gamePieceTypeId
                WHERE gp.releaseKey = lr.releaseKey AND gpt.type = 'meta'
                ORDER BY gp.userId DESC, gp.languageId ASC
                LIMIT 1
              ) AS meta_raw,
              (
                SELECT gt.minutesInGame
                FROM GameTimes gt
                WHERE gt.releaseKey = lr.releaseKey
                ORDER BY gt.userId DESC
                LIMIT 1
              ) AS playtime_minutes,
              (
                SELECT lpd.lastPlayedDate
                FROM LastPlayedDates lpd
                WHERE lpd.gameReleaseKey = lr.releaseKey
                ORDER BY lpd.userId DESC
                LIMIT 1
              ) AS last_played_at,
              (
                SELECT d.releaseDate
                FROM Details d
                JOIN LimitedDetails ld ON ld.id = d.limitedDetailsId
                JOIN ProductsToReleaseKeys ptr3 ON ptr3.gogId = ld.productId
                WHERE ptr3.releaseKey = lr.releaseKey
                ORDER BY ld.stored_at DESC
                LIMIT 1
              ) AS details_release_raw
            FROM LibraryReleases lr
            JOIN LicensedReleases lz
              ON lz.libraryId = lr.id
             AND lz.isOwned = 1
            LEFT JOIN ProductsToReleaseKeys ptr
              ON ptr.releaseKey = lr.releaseKey
            WHERE lr.releaseKey LIKE 'gog_%'
              AND (ptr.externalId IS NULL OR ptr.releaseKey IS NULL)
            ORDER BY lr.releaseKey
            "#,
        )
        .fetch_all(&source_pool)
        .await
        {
            Ok(v) => v,
            Err(_) => continue,
        };

        if rows.is_empty() {
            continue;
        }

        let mut picked = std::collections::HashMap::<String, GogImportCandidate>::new();
        for r in rows {
            let release_key: String = r.try_get("release_key").unwrap_or_default();
            if release_key.is_empty() {
                continue;
            }

            let title_raw = r.try_get::<Option<String>, _>("title_raw").unwrap_or(None);
            let meta_raw = r.try_get::<Option<String>, _>("meta_raw").unwrap_or(None);
            let playtime_minutes = r
                .try_get::<Option<i64>, _>("playtime_minutes")
                .unwrap_or(None);
            let last_played_raw = r
                .try_get::<Option<String>, _>("last_played_at")
                .unwrap_or(None);
            let details_release_raw = r
                .try_get::<Option<String>, _>("details_release_raw")
                .unwrap_or(None);

            let Some(title_raw) = parse_gog_title(title_raw.as_deref()) else {
                continue;
            };
            let (title, is_variant) = strip_marketplace_suffixes(&title_raw);
            if title.is_empty() {
                continue;
            }

            let (meta_year, meta_genres) = parse_gog_meta(meta_raw.as_deref());
            let release_year =
                meta_year.or_else(|| parse_year_from_text(details_release_raw.as_deref()));
            let genre = meta_genres.map(|v| v.join(", "));
            let last_played_at = normalize_sqlite_datetime_to_iso(last_played_raw.as_deref());

            let mut score = 0_i32;
            if !is_variant {
                score += 8;
            }
            if release_year.is_some() {
                score += 2;
            }
            if genre
                .as_ref()
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false)
            {
                score += 2;
            }
            if playtime_minutes.unwrap_or(0) > 0 {
                score += 1;
            }
            if last_played_at.is_some() {
                score += 1;
            }

            let key = normalize_title_for_dedupe(&title);
            if key.is_empty() {
                continue;
            }

            let cand = GogImportCandidate {
                release_key,
                title,
                playtime_minutes,
                last_played_at,
                genre,
                release_year,
                score,
            };

            match picked.get_mut(&key) {
                Some(existing) => {
                    if cand.score > existing.score
                        || (cand.score == existing.score && cand.title.len() < existing.title.len())
                    {
                        *existing = cand;
                    }
                }
                None => {
                    picked.insert(key, cand);
                }
            }
        }

        if picked.is_empty() {
            continue;
        }

        let mut candidates: Vec<GogImportCandidate> = picked.into_values().collect();
        candidates.sort_by(|a, b| {
            a.title
                .to_ascii_lowercase()
                .cmp(&b.title.to_ascii_lowercase())
                .then_with(|| a.release_key.cmp(&b.release_key))
        });
        return Ok((db_file, candidates));
    }

    Err(format!(
        "Could not read GOG owned titles from SQLite. Tried: {}",
        tried.join(", ")
    ))
}

#[tauri::command]
pub async fn import_gog_owned_from_sqlite(
    db: tauri::State<'_, Db>,
    db_path: Option<String>,
) -> Result<u32, String> {
    #[derive(Clone)]
    struct GogExistingRow {
        id: String,
        title: String,
        cleaned_title: String,
        is_variant: bool,
    }

    let (db_file, candidates) = load_gog_import_candidates(db_path.as_deref()).await?;
    let mut imported: u32 = 0;

    for cand in &candidates {
        sqlx::query(
            r#"
            INSERT INTO games (
              id, canonical_title, platform, storefront_id, owned_source,
              playtime_minutes, last_played_at, genre, release_year,
              license_type, license_source, updated_at
            )
            VALUES (?, ?, 'GOG', ?, 'GOG Galaxy DB', ?, ?, ?, ?, 'owned', NULL, CURRENT_TIMESTAMP)
            ON CONFLICT(platform, storefront_id) DO UPDATE SET
              canonical_title  = excluded.canonical_title,
              playtime_minutes = COALESCE(excluded.playtime_minutes, games.playtime_minutes),
              last_played_at   = COALESCE(excluded.last_played_at, games.last_played_at),
              genre            = COALESCE(excluded.genre, games.genre),
              release_year     = COALESCE(excluded.release_year, games.release_year),
              owned_source     = excluded.owned_source,
              license_type     = 'owned',
              license_source   = NULL,
              updated_at       = CURRENT_TIMESTAMP
            "#,
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&cand.title)
        .bind(&cand.release_key)
        .bind(cand.playtime_minutes)
        .bind(cand.last_played_at.clone())
        .bind(cand.genre.clone())
        .bind(cand.release_year)
        .execute(db.inner())
        .await
        .map_err(|e| e.to_string())?;

        imported += 1;
    }

    // Cleanup historical duplicates that used marketplace-suffixed variants.
    let existing_rows = sqlx::query(
        "SELECT id, canonical_title FROM games WHERE platform = 'GOG' ORDER BY canonical_title",
    )
    .fetch_all(db.inner())
    .await
    .map_err(|e| e.to_string())?;

    let mut groups = std::collections::HashMap::<String, Vec<GogExistingRow>>::new();
    for row in existing_rows {
        let id: String = row.try_get(0).unwrap_or_default();
        let title: String = row.try_get(1).unwrap_or_default();
        if id.is_empty() {
            continue;
        }
        let (cleaned_title, is_variant) = strip_marketplace_suffixes(&title);
        if cleaned_title.is_empty() {
            continue;
        }
        let key = normalize_title_for_dedupe(&cleaned_title);
        if key.is_empty() {
            continue;
        }
        groups.entry(key).or_default().push(GogExistingRow {
            id,
            title,
            cleaned_title,
            is_variant,
        });
    }

    let mut delete_ids: Vec<String> = Vec::new();
    let mut rename_ops: Vec<(String, String)> = Vec::new();
    for group in groups.values() {
        if group.is_empty() {
            continue;
        }

        let has_non_variant = group.iter().any(|g| !g.is_variant);
        if has_non_variant {
            for row in group.iter().filter(|g| g.is_variant) {
                delete_ids.push(row.id.clone());
            }
            continue;
        }

        // Only variants exist for this normalized title.
        let mut sorted = group.clone();
        sorted.sort_by(|a, b| {
            a.title
                .len()
                .cmp(&b.title.len())
                .then_with(|| a.id.cmp(&b.id))
        });
        if let Some(keep) = sorted.first() {
            if keep.title != keep.cleaned_title {
                rename_ops.push((keep.id.clone(), keep.cleaned_title.clone()));
            }
            for row in sorted.iter().skip(1) {
                delete_ids.push(row.id.clone());
            }
        }
    }

    for (id, new_title) in rename_ops {
        sqlx::query(
            "UPDATE games SET canonical_title = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
        )
        .bind(new_title)
        .bind(id)
        .execute(db.inner())
        .await
        .map_err(|e| e.to_string())?;
    }
    for id in delete_ids.iter() {
        sqlx::query("DELETE FROM games WHERE id = ?")
            .bind(id)
            .execute(db.inner())
            .await
            .map_err(|e| e.to_string())?;
    }

    if !delete_ids.is_empty() {
        println!(
            "[import] GOG cleanup removed {} variant duplicate row(s).",
            delete_ids.len()
        );
    }

    println!(
        "[import] GOG owned import finished. Imported/updated {} rows.",
        imported
    );
    logging::log_db_transaction(format!(
        "GOG import completed source_db={} imported_or_updated={} variant_duplicates_removed={}",
        quote_for_log(&db_file.display().to_string(), 160),
        imported,
        delete_ids.len()
    ));
    Ok(imported)
}

#[tauri::command]
pub async fn sync_gog_playtime_from_sqlite(
    db: tauri::State<'_, Db>,
    db_path: Option<String>,
) -> Result<u32, String> {
    let (db_file, candidates) = load_gog_import_candidates(db_path.as_deref()).await?;
    let mut tx = db.begin().await.map_err(|e| e.to_string())?;
    let mut synced = 0_u32;

    for cand in &candidates {
        let result = sqlx::query(
            r#"
            UPDATE games
            SET playtime_minutes = COALESCE(?, playtime_minutes),
                last_played_at = COALESCE(?, last_played_at),
                updated_at = CURRENT_TIMESTAMP
            WHERE platform = 'GOG' AND storefront_id = ?
            "#,
        )
        .bind(cand.playtime_minutes)
        .bind(cand.last_played_at.clone())
        .bind(&cand.release_key)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

        synced += result.rows_affected() as u32;
    }

    tx.commit().await.map_err(|e| e.to_string())?;
    logging::log_db_transaction(format!(
        "GOG playtime sync completed source_db={} matched_or_updated={}",
        quote_for_log(&db_file.display().to_string(), 160),
        synced
    ));
    Ok(synced)
}

// ========================= Manual add =========================

#[derive(Serialize, Deserialize)]
pub struct NewManualGame {
    #[serde(rename = "canonicalTitle")]
    pub canonical_title: String,
    pub platform: String,
    pub beaten: Option<bool>,
    pub genre: Option<String>,

    // Optional license fields
    #[serde(rename = "licenseType")]
    pub license_type: Option<String>, // "owned" | "subscription" | "free" | "trial"
    #[serde(rename = "licenseSource")]
    pub license_source: Option<String>, // "PC Game Pass", "Humble Choice", …
}

#[tauri::command]
pub async fn add_manual_game(
    db: tauri::State<'_, Db>,
    payload: NewManualGame,
) -> Result<String, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let license_type = normalize_license_type(payload.license_type.as_deref())?;
    let license_source = normalize_license_source(license_type, payload.license_source.as_deref());

    sqlx::query(
        r#"
        INSERT INTO games (
          id, canonical_title, alt_titles,
          platform, owned_source, beaten, genre,
          license_type, license_source
        )
        VALUES (?, ?, '[]', ?, 'Manual', ?, ?, ?, ?)
        "#,
    )
    .bind(&id)
    .bind(&payload.canonical_title)
    .bind(&payload.platform)
    .bind(payload.beaten.unwrap_or(false) as i64)
    .bind(&payload.genre)
    .bind(license_type)
    .bind(&license_source)
    .execute(db.inner())
    .await
    .map_err(|e| e.to_string())?;

    logging::log_db_transaction(format!(
        "Manual game added {} beaten={} genre={} license_type={} license_source={}",
        format_game_identity(&payload.canonical_title, &payload.platform, &id),
        payload.beaten.unwrap_or(false),
        format_optional_text_for_log(payload.genre.as_deref(), 96),
        quote_for_log(license_type, 24),
        format_optional_text_for_log(license_source.as_deref(), 64),
    ));

    Ok(id)
}

// ========================= List rows for UI =========================

#[derive(Serialize)]
pub struct UiGameRow {
    pub id: String,
    pub title: String,
    pub platform: String,
    pub beaten: bool,
    pub genre: Option<String>,

    pub playtime_minutes: Option<i64>,
    pub last_played_at: Option<String>,
    pub cover_url: Option<String>,
    pub release_year: Option<i64>,
    pub enriched_at: Option<String>,
    pub hidden_at: Option<String>,
    pub queue_position: Option<i64>,
    pub queue_is_playing: bool,
    pub queue_started_at: Option<String>,
    pub queue_finished_at: Option<String>,

    pub license_type: Option<String>,
    pub license_source: Option<String>,
    pub review_rating: Option<i64>,
    pub has_review: bool,
    pub ttb_main: Option<i64>,
    pub ttb_extra: Option<i64>,
    pub ttb_complete: Option<i64>,
    pub hltb_main: Option<i64>,
    pub hltb_extra: Option<i64>,
    pub hltb_complete: Option<i64>,
}

#[tauri::command]
pub async fn list_games(db: tauri::State<'_, Db>) -> Result<Vec<UiGameRow>, String> {
    let rows = sqlx::query(
        r#"
        SELECT
          id,
          canonical_title,
          platform,
          beaten,
          genre,
          playtime_minutes,
          last_played_at,
          cover_url,
          CAST(release_year AS INTEGER) AS release_year,
          enriched_at,
          hidden_at,
          queue_position,
          queue_is_playing,
          queue_started_at,
          queue_finished_at,
          license_type,
          license_source,
          review_rating,
          ttb_main,
          ttb_extra,
          ttb_complete,
          hltb_main,
          hltb_extra,
          hltb_complete,
          CASE
            WHEN review_rating IS NOT NULL OR TRIM(COALESCE(review_text, '')) <> '' THEN 1
            ELSE 0
          END AS has_review
        FROM games
        ORDER BY canonical_title COLLATE NOCASE, canonical_title, platform, id
        "#,
    )
    .fetch_all(db.inner())
    .await
    .map_err(|e| e.to_string())?;

    let data = rows
        .into_iter()
        .map(|r| {
            let id: String = r.get(0);
            let title: String = r.get(1);
            let platform: String = r.get(2);
            let beaten_i: Option<i64> = r.get(3);
            let genre: Option<String> = r.get(4);
            let playtime: Option<i64> = r.get(5);
            let last_played: Option<String> = r.get(6);
            let cover_url: Option<String> = r.get(7);
            let release_year: Option<i64> = r.get(8);
            let enriched_at: Option<String> = r.get(9);
            let hidden_at: Option<String> = r.get(10);
            let queue_position: Option<i64> = r.get(11);
            let queue_is_playing_i: Option<i64> = r.get(12);
            let queue_started_at: Option<String> = r.get(13);
            let queue_finished_at: Option<String> = r.get(14);
            let license_type: Option<String> = r.get(15);
            let license_source: Option<String> = r.get(16);
            let review_rating: Option<i64> = r.get(17);
            let ttb_main: Option<i64> = r.get(18);
            let ttb_extra: Option<i64> = r.get(19);
            let ttb_complete: Option<i64> = r.get(20);
            let hltb_main: Option<i64> = r.get(21);
            let hltb_extra: Option<i64> = r.get(22);
            let hltb_complete: Option<i64> = r.get(23);
            let has_review_i: Option<i64> = r.get(24);

            UiGameRow {
                id,
                title,
                platform,
                beaten: beaten_i.unwrap_or(0) != 0,
                genre,
                playtime_minutes: playtime,
                last_played_at: last_played,
                cover_url,
                release_year,
                enriched_at,
                hidden_at,
                queue_position,
                queue_is_playing: queue_is_playing_i.unwrap_or(0) != 0,
                queue_started_at,
                queue_finished_at,
                license_type,
                license_source,
                review_rating,
                has_review: has_review_i.unwrap_or(0) != 0,
                ttb_main,
                ttb_extra,
                ttb_complete,
                hltb_main,
                hltb_extra,
                hltb_complete,
            }
        })
        .collect();

    Ok(data)
}

// ========================= Steam import =========================

#[derive(Deserialize)]
pub struct SteamImportPayload {
    pub api_key: String, // may be empty: XML fallback
    pub profile: String, // vanity, steamid64, or full profile URL
}

async fn fetch_steam_owned_games_for_payload(
    payload: &SteamImportPayload,
) -> Result<(Vec<crate::connectors::steam::SteamGame>, &'static str), String> {
    use crate::connectors::steam::{
        fetch_owned_games, fetch_owned_games_via_xml, resolve_to_steamid,
    };

    if payload.api_key.trim().is_empty() {
        let games = fetch_owned_games_via_xml(&payload.profile)
            .await
            .map_err(|e| e.to_string())?;
        Ok((games, "steam-community-xml"))
    } else {
        let steamid = resolve_to_steamid(&payload.api_key, &payload.profile)
            .await
            .map_err(|e| e.to_string())?;
        let games = fetch_owned_games(&payload.api_key, &steamid)
            .await
            .map_err(|e| e.to_string())?;
        Ok((games, "steam-api"))
    }
}

#[tauri::command]
pub async fn import_steam_games(
    db: tauri::State<'_, Db>,
    payload: SteamImportPayload,
) -> Result<u32, String> {
    use crate::connectors::steam::{steam_header_image, unix_to_iso};

    let (games, import_source) = fetch_steam_owned_games_for_payload(&payload).await?;
    let mut imported: u32 = 0;
    for g in games {
        let appid = g.appid.to_string();
        let title = g.name;
        let play_min = g.playtime_forever.unwrap_or(0) as i64;

        // Store NULL when never played (0), avoid epoch 1970 date
        let last_played_iso =
            g.rtime_last_played
                .and_then(|ts| if ts > 0 { Some(unix_to_iso(ts)) } else { None });

        let cover = steam_header_image(g.appid);

        sqlx::query(
            r#"
            INSERT INTO games (
              id, canonical_title, platform, storefront_id, owned_source,
              playtime_minutes, last_played_at, cover_url, updated_at
            )
            VALUES (?, ?, 'Steam', ?, 'Steam API/Community', ?, ?, ?, CURRENT_TIMESTAMP)
            ON CONFLICT(platform, storefront_id) DO UPDATE SET
              canonical_title   = excluded.canonical_title,
              playtime_minutes  = excluded.playtime_minutes,
              last_played_at    = excluded.last_played_at,
              cover_url         = excluded.cover_url,
              updated_at        = CURRENT_TIMESTAMP
            "#,
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&title)
        .bind(&appid)
        .bind(play_min)
        .bind(&last_played_iso)
        .bind(&cover)
        .execute(db.inner())
        .await
        .map_err(|e| e.to_string())?;

        imported += 1;
    }

    println!(
        "[import] Finished Steam import. Imported/updated {} rows.",
        imported
    );
    logging::log_db_transaction(format!(
        "Steam import completed source={} profile={} imported_or_updated={}",
        import_source,
        quote_for_log(&payload.profile, 96),
        imported
    ));
    Ok(imported)
}

#[tauri::command]
pub async fn sync_steam_playtime(
    db: tauri::State<'_, Db>,
    payload: SteamImportPayload,
) -> Result<u32, String> {
    use crate::connectors::steam::unix_to_iso;

    let (games, import_source) = fetch_steam_owned_games_for_payload(&payload).await?;
    let mut tx = db.begin().await.map_err(|e| e.to_string())?;
    let mut synced = 0_u32;

    for g in games {
        let appid = g.appid.to_string();
        let play_min = g.playtime_forever.unwrap_or(0) as i64;
        let last_played_iso =
            g.rtime_last_played
                .and_then(|ts| if ts > 0 { Some(unix_to_iso(ts)) } else { None });

        let result = sqlx::query(
            r#"
            UPDATE games
            SET playtime_minutes = ?,
                last_played_at = COALESCE(?, last_played_at),
                updated_at = CURRENT_TIMESTAMP
            WHERE platform = 'Steam' AND storefront_id = ?
            "#,
        )
        .bind(play_min)
        .bind(last_played_iso)
        .bind(&appid)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

        synced += result.rows_affected() as u32;
    }

    tx.commit().await.map_err(|e| e.to_string())?;
    logging::log_db_transaction(format!(
        "Steam playtime sync completed source={} profile={} matched_or_updated={}",
        import_source,
        quote_for_log(&payload.profile, 96),
        synced
    ));
    Ok(synced)
}

// ========================= IGDB helpers =========================

#[derive(Serialize)]
pub struct IgdbGenre {
    pub id: i64,
    pub name: String,
}

#[tauri::command]
pub async fn get_igdb_genres() -> Result<Vec<IgdbGenre>, String> {
    const NAMES: &[&str] = &[
        "Action",
        "Adventure",
        "RPG",
        "Shooter",
        "Strategy",
        "Puzzle",
        "Racing",
        "Sports",
        "Simulation",
        "Platform",
        "Hack and slash/Beat 'em up",
        "Indie",
        "Arcade",
        "Visual Novel",
        "Tactical",
        "RTS",
        "Turn-based strategy",
        "MOBA",
        "Fighting",
        "Music",
        "Rhythm",
        "Quiz/Trivia",
        "Pinball",
        "Roguelike",
        "Sandbox",
        "Survival",
        "Stealth",
        "Point-and-click",
        "Card & Board Game",
        "Battle Royale",
        "Tactical RPG",
        "JRPG",
        "Western RPG",
        "Metroidvania",
        "Party",
        "City Builder",
    ];
    Ok(NAMES
        .iter()
        .enumerate()
        .map(|(i, n)| IgdbGenre {
            id: i as i64 + 1,
            name: n.to_string(),
        })
        .collect())
}

#[derive(Debug, Clone, Serialize)]
struct ProgressPayload {
    done: u32,
    total: u32,
    title: String,
}

/// Emit IGDB progress to the frontend; prefer the "main" window, fall back to broadcast.
#[inline]
fn emit_igdb_progress(app: &tauri::AppHandle, done: u32, total: u32, title: impl Into<String>) {
    let payload = ProgressPayload {
        done,
        total,
        title: title.into(),
    };
    if let Err(e1) = app.emit_to("main", "igdb_enrich_progress", payload.clone()) {
        eprintln!("[emit_to main failed] {e1}");
        if let Err(e2) = app.emit("igdb_enrich_progress", payload) {
            eprintln!("[emit broadcast failed] {e2}");
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IgdbAuth {
    pub client_id: String,
    pub client_secret: String,
}

#[tauri::command]
pub async fn enrich_genres_with_igdb(
    app: tauri::AppHandle,
    db: tauri::State<'_, Db>,
    auth: IgdbAuth,
    only_unenriched: Option<bool>,
) -> Result<u32, String> {
    use crate::services::hltb::lookup_best_for_title;
    use crate::services::igdb::{get_oauth_token, lookup_enrich_for_title};
    use tokio::time::{sleep, Duration};

    let only_unenriched = only_unenriched.unwrap_or(false);
    let run_id = uuid::Uuid::new_v4().to_string();
    let run_tag = run_id.get(..8).unwrap_or(run_id.as_str()).to_string();

    // 1) get OAuth token (retry on 429)
    let token = loop {
        match get_oauth_token(&auth.client_id, &auth.client_secret).await {
            Ok(t) => break t,
            Err(e) => {
                if e.to_string().contains("429") {
                    sleep(Duration::from_secs(2)).await;
                    continue;
                } else {
                    logging::log_enrichment(format!(
                        "[run:{}] failed_to_acquire_igdb_token error={}",
                        run_tag,
                        quote_for_log(&e.to_string(), 180)
                    ));
                    return Err(e.to_string());
                }
            }
        }
    };

    // 2) count + list targets
    let where_clause = if only_unenriched {
        "hidden_at IS NULL AND enriched_at IS NULL"
    } else {
        "hidden_at IS NULL AND (
           (genre IS NULL OR TRIM(genre) = '')
           OR (cover_url IS NULL OR TRIM(cover_url) = '')
           OR (release_year IS NULL)
           OR (summary IS NULL OR TRIM(summary) = '')
           OR (agg_rating IS NULL)
           OR (user_rating IS NULL)
           OR (ttb_main IS NULL OR ttb_extra IS NULL OR ttb_complete IS NULL)
           OR (igdb_url IS NULL OR TRIM(igdb_url) = '')
           OR (hltb_id IS NULL OR hltb_main IS NULL OR hltb_extra IS NULL OR hltb_complete IS NULL)
         )"
    };

    let count_sql = format!("SELECT COUNT(*) FROM games WHERE {}", where_clause);
    let total: i64 = sqlx::query_scalar::<sqlx::Sqlite, i64>(&count_sql)
        .fetch_one(db.inner())
        .await
        .map_err(|e| e.to_string())?;

    let rows_sql = format!(
        r#"
        SELECT id, canonical_title, platform, genre, cover_url, release_year,
               summary, storyline, agg_rating, user_rating,
               ttb_main, ttb_extra, ttb_complete, ttb_count, igdb_id, igdb_url, enriched_at,
               hltb_id, hltb_main, hltb_extra, hltb_complete
        FROM games
        WHERE {}
        ORDER BY canonical_title
        "#,
        where_clause
    );
    let rows = sqlx::query(&rows_sql)
        .fetch_all(db.inner())
        .await
        .map_err(|e| e.to_string())?;

    let total_u32 = total as u32;
    let mut updated: u32 = 0;
    let mut done: u32 = 0;

    emit_igdb_progress(&app, done, total_u32, "starting");
    println!("[IGDB] Starting enrichment for {} item(s)", total);
    logging::log_enrichment(format!(
        "[run:{}] enrichment_started mode={} total={}",
        run_tag,
        if only_unenriched {
            "unenriched-only"
        } else {
            "missing-fields"
        },
        total_u32
    ));

    for r in rows {
        let id: String = r.get(0);
        let title: String = r.get(1);
        let platform: String = r.get(2);
        let current_genre: Option<String> = r.get(3);
        let current_cover: Option<String> = r.get(4);
        let current_year: Option<i64> = r.get(5);
        let current_summary: Option<String> = r.get(6);
        let current_story: Option<String> = r.get(7);
        let current_agg: Option<f64> = r.get(8);
        let current_user: Option<f64> = r.get(9);
        let current_ttb_main: Option<i64> = r.get(10);
        let current_ttb_extra: Option<i64> = r.get(11);
        let current_ttb_complete: Option<i64> = r.get(12);
        let _current_ttb_count: Option<i64> = r.get(13);
        let current_igdb_id: Option<i64> = r.get(14);
        let current_igdb_url: Option<String> = r.get(15);
        let current_enriched_at: Option<String> = r.get(16);
        let current_hltb_id: Option<i64> = r.get(17);
        let current_hltb_main: Option<i64> = r.get(18);
        let current_hltb_extra: Option<i64> = r.get(19);
        let current_hltb_complete: Option<i64> = r.get(20);

        let need_genre = current_genre
            .as_ref()
            .map(|s| s.trim().is_empty())
            .unwrap_or(true);
        let need_cover = current_cover
            .as_ref()
            .map(|s| s.trim().is_empty())
            .unwrap_or(true);
        let need_year = current_year.is_none();
        let need_summary = current_summary
            .as_ref()
            .map(|s| s.trim().is_empty())
            .unwrap_or(true);
        let need_story = current_story
            .as_ref()
            .map(|s| s.trim().is_empty())
            .unwrap_or(true);
        let need_agg = current_agg.is_none();
        let need_user = current_user.is_none();
        let need_ttb = current_ttb_main.is_none()
            || current_ttb_extra.is_none()
            || current_ttb_complete.is_none();
        let need_igdb_url = current_igdb_url
            .as_ref()
            .map(|s| s.trim().is_empty())
            .unwrap_or(true);
        let need_hltb = current_hltb_id.is_none()
            || current_hltb_main.is_none()
            || current_hltb_extra.is_none()
            || current_hltb_complete.is_none();
        let game_identity = format_game_identity(&title, &platform, &id);
        let mut missing_fields: Vec<&'static str> = Vec::new();
        if need_genre {
            missing_fields.push("genre");
        }
        if need_cover {
            missing_fields.push("cover_url");
        }
        if need_year {
            missing_fields.push("release_year");
        }
        if need_summary {
            missing_fields.push("summary");
        }
        if need_story {
            missing_fields.push("storyline");
        }
        if need_agg {
            missing_fields.push("agg_rating");
        }
        if need_user {
            missing_fields.push("user_rating");
        }
        if need_ttb {
            missing_fields.push("igdb_ttb");
        }
        if need_igdb_url {
            missing_fields.push("igdb_url");
        }
        if need_hltb {
            missing_fields.push("hltb");
        }
        let missing_fields_display = if missing_fields.is_empty() {
            "none".to_string()
        } else {
            missing_fields.join("|")
        };

        emit_igdb_progress(&app, done, total_u32, title.clone());
        println!("[IGDB] ({}/{}) {}", done, total_u32, title);

        if !need_genre
            && !need_cover
            && !need_year
            && !need_summary
            && !need_story
            && !need_agg
            && !need_user
            && !need_ttb
            && !need_igdb_url
            && !need_hltb
        {
            let mut status = "already_complete";
            let mut updated_fields = "none".to_string();
            if only_unenriched && current_enriched_at.is_none() {
                sqlx::query("UPDATE games SET enriched_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
                    .bind(&id)
                    .execute(db.inner())
                    .await
                    .map_err(|e| e.to_string())?;
                status = "marked_enriched_without_field_changes";
                updated_fields = "enriched_at".to_string();
            }
            logging::log_enrichment(format!(
                "[run:{}] {} status={} missing={} updated_fields={}",
                run_tag, game_identity, status, missing_fields_display, updated_fields
            ));
            done += 1;
            emit_igdb_progress(&app, done, total_u32, title.clone());
            continue;
        }

        let mut did_any = false;
        let mut had_error = false;
        let mut updated_fields: Vec<&'static str> = Vec::new();
        let mut hltb_status = if need_hltb {
            "pending".to_string()
        } else {
            "not-needed".to_string()
        };

        let igdb_status = match lookup_enrich_for_title(&auth.client_id, &token, &title).await {
            Ok(Some(info)) => {
                if current_igdb_id != Some(info.igdb_id) {
                    sqlx::query(
                        "UPDATE games SET igdb_id = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
                    )
                    .bind(info.igdb_id)
                    .bind(&id)
                    .execute(db.inner())
                    .await
                    .map_err(|e| e.to_string())?;
                    did_any = true;
                    updated_fields.push("igdb_id");
                }

                if let Some(url) = info
                    .igdb_url
                    .as_deref()
                    .filter(|url| !url.trim().is_empty())
                {
                    let current_url = current_igdb_url
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty());
                    if current_igdb_id != Some(info.igdb_id) || current_url != Some(url) {
                        sqlx::query("UPDATE games SET igdb_url = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
                            .bind(url)
                            .bind(&id)
                            .execute(db.inner())
                            .await
                            .map_err(|e| e.to_string())?;
                        did_any = true;
                        updated_fields.push("igdb_url");
                    }
                }

                if need_genre && !info.genres.is_empty() {
                    let joined = info.genres.join(", ");
                    sqlx::query(
                        "UPDATE games SET genre = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
                    )
                    .bind(&joined)
                    .bind(&id)
                    .execute(db.inner())
                    .await
                    .map_err(|e| e.to_string())?;
                    did_any = true;
                    updated_fields.push("genre");
                }

                if need_cover {
                    if let Some(url) = info.cover_url.as_deref() {
                        if !url.is_empty() {
                            sqlx::query("UPDATE games SET cover_url = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
                                .bind(url).bind(&id)
                                .execute(db.inner()).await.map_err(|e| e.to_string())?;
                            did_any = true;
                            updated_fields.push("cover_url");
                        }
                    }
                }

                if need_year {
                    if let Some(y) = info.release_year {
                        if y > 0 {
                            sqlx::query("UPDATE games SET release_year = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
                                .bind(y).bind(&id)
                                .execute(db.inner()).await.map_err(|e| e.to_string())?;
                            did_any = true;
                            updated_fields.push("release_year");
                        }
                    }
                }

                if need_summary {
                    if let Some(s) = info
                        .summary
                        .as_ref()
                        .map(|s| s.trim())
                        .filter(|s| !s.is_empty())
                    {
                        sqlx::query("UPDATE games SET summary = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
                            .bind(s).bind(&id)
                            .execute(db.inner()).await.map_err(|e| e.to_string())?;
                        did_any = true;
                        updated_fields.push("summary");
                    }
                }

                if need_story {
                    if let Some(s) = info
                        .storyline
                        .as_ref()
                        .map(|s| s.trim())
                        .filter(|s| !s.is_empty())
                    {
                        sqlx::query("UPDATE games SET storyline = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
                            .bind(s).bind(&id)
                            .execute(db.inner()).await.map_err(|e| e.to_string())?;
                        did_any = true;
                        updated_fields.push("storyline");
                    }
                }

                if need_agg {
                    if let Some(v) = info.agg_rating {
                        sqlx::query("UPDATE games SET agg_rating = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
                            .bind(v).bind(&id)
                            .execute(db.inner()).await.map_err(|e| e.to_string())?;
                        did_any = true;
                        updated_fields.push("agg_rating");
                    }
                }
                if need_user {
                    if let Some(v) = info.user_rating {
                        sqlx::query("UPDATE games SET user_rating = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
                            .bind(v).bind(&id)
                            .execute(db.inner()).await.map_err(|e| e.to_string())?;
                        did_any = true;
                        updated_fields.push("user_rating");
                    }
                }

                if need_ttb {
                    if info.ttb_main.is_some()
                        || info.ttb_extra.is_some()
                        || info.ttb_complete.is_some()
                    {
                        sqlx::query(
                            "UPDATE games
                             SET ttb_main = ?, ttb_extra = ?, ttb_complete = ?, ttb_count = ?, updated_at = CURRENT_TIMESTAMP
                             WHERE id = ?"
                        )
                        .bind(info.ttb_main)
                        .bind(info.ttb_extra)
                        .bind(info.ttb_complete)
                        .bind(info.ttb_count)
                        .bind(&id)
                        .execute(db.inner()).await.map_err(|e| e.to_string())?;
                        did_any = true;
                        updated_fields.push("igdb_ttb");
                    }
                }

                sleep(Duration::from_millis(900)).await;
                format!("matched igdb_id={}", info.igdb_id)
            }
            Ok(None) => {
                sleep(Duration::from_millis(500)).await;
                "no-match".to_string()
            }
            Err(err) => {
                let err_text = err.to_string();
                had_error = true;
                if err_text.contains("429") {
                    sleep(Duration::from_secs(2)).await;
                } else {
                    sleep(Duration::from_millis(400)).await;
                }
                format!("error {}", quote_for_log(&err_text, 140))
            }
        };

        if need_hltb {
            match lookup_best_for_title(&title, Some(&platform)).await {
                Ok(Some(info)) => {
                    let hltb_changed = current_hltb_id != Some(info.hltb_id)
                        || current_hltb_main != info.hltb_main
                        || current_hltb_extra != info.hltb_extra
                        || current_hltb_complete != info.hltb_complete;

                    if hltb_changed {
                        sqlx::query(
                            "UPDATE games
                             SET hltb_id = ?,
                                 hltb_title = ?,
                                 hltb_url = ?,
                                 hltb_main = ?,
                                 hltb_extra = ?,
                                 hltb_complete = ?,
                                 hltb_all = ?,
                                 hltb_main_count = ?,
                                 hltb_extra_count = ?,
                                 hltb_complete_count = ?,
                                 hltb_all_count = ?,
                                 hltb_platforms = ?,
                                 hltb_genres = ?,
                                 hltb_summary = ?,
                                 hltb_match_method = ?,
                                 hltb_updated_at = CURRENT_TIMESTAMP,
                                 updated_at = CURRENT_TIMESTAMP
                             WHERE id = ?",
                        )
                        .bind(info.hltb_id)
                        .bind(&info.title)
                        .bind(&info.profile_url)
                        .bind(info.hltb_main)
                        .bind(info.hltb_extra)
                        .bind(info.hltb_complete)
                        .bind(info.hltb_all)
                        .bind(info.hltb_main_count)
                        .bind(info.hltb_extra_count)
                        .bind(info.hltb_complete_count)
                        .bind(info.hltb_all_count)
                        .bind(&info.platforms)
                        .bind(&info.genres)
                        .bind(&info.summary)
                        .bind(&info.match_method)
                        .bind(&id)
                        .execute(db.inner())
                        .await
                        .map_err(|e| e.to_string())?;
                        did_any = true;
                        updated_fields.push("hltb");
                        hltb_status = format!("matched hltb_id={}", info.hltb_id);
                    } else {
                        hltb_status = format!("matched_no_change hltb_id={}", info.hltb_id);
                    }
                }
                Ok(None) => {
                    hltb_status = "no-match".to_string();
                }
                Err(err) => {
                    eprintln!("[HLTB] lookup failed for '{}': {}", title, err);
                    hltb_status = format!("error {}", quote_for_log(&err.to_string(), 140));
                    had_error = true;
                }
            }
        }

        if did_any {
            sqlx::query("UPDATE games SET enriched_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
                .bind(&id)
                .execute(db.inner())
                .await
                .map_err(|e| e.to_string())?;
            updated += 1;
        }

        let status = if did_any {
            if had_error {
                "updated_with_errors"
            } else {
                "updated"
            }
        } else if had_error {
            "completed_with_errors"
        } else if igdb_status == "no-match" && (!need_hltb || hltb_status == "no-match") {
            "no_data_found"
        } else {
            "no_changes_applied"
        };
        let updated_fields_display = if updated_fields.is_empty() {
            "none".to_string()
        } else {
            updated_fields.join("|")
        };
        logging::log_enrichment(format!(
            "[run:{}] {} status={} missing={} updated_fields={} igdb_status={} hltb_status={}",
            run_tag,
            game_identity,
            status,
            missing_fields_display,
            updated_fields_display,
            igdb_status,
            hltb_status
        ));

        done += 1;
        emit_igdb_progress(&app, done, total_u32, title.clone());
    }

    emit_igdb_progress(&app, total_u32, total_u32, "done");
    println!(
        "[IGDB] Finished {} run. Rows updated: {}",
        if only_unenriched {
            "(unenriched-only)"
        } else {
            "(missing-fields)"
        },
        updated
    );
    logging::log_enrichment(format!(
        "[run:{}] enrichment_finished mode={} rows_updated={}",
        run_tag,
        if only_unenriched {
            "unenriched-only"
        } else {
            "missing-fields"
        },
        updated
    ));
    Ok(updated)
}

// ========================= Twitch creds =========================

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TwitchCreds {
    pub client_id: String,
    pub client_secret: String,
}

#[tauri::command]
pub async fn save_twitch_credentials(creds: TwitchCreds) -> Result<(), String> {
    crate::security::twitch::save_twitch(&creds.client_id, &creds.client_secret)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_twitch_credentials() -> Result<TwitchCreds, String> {
    let (id, secret) = crate::security::twitch::load_twitch().map_err(|e| e.to_string())?;
    Ok(TwitchCreds {
        client_id: id.unwrap_or_default(),
        client_secret: secret.unwrap_or_default(),
    })
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SteamCreds {
    pub api_key: String,
    pub profile: String,
}

#[tauri::command]
pub async fn save_steam_credentials(creds: SteamCreds) -> Result<(), String> {
    crate::security::steam::save_steam(&creds.api_key, &creds.profile).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_steam_credentials() -> Result<SteamCreds, String> {
    let (api_key, profile) = crate::security::steam::load_steam().map_err(|e| e.to_string())?;
    Ok(SteamCreds {
        api_key: api_key.unwrap_or_default(),
        profile: profile.unwrap_or_default(),
    })
}

// ========================= Update / details / notes =========================

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct GameUpdate {
    pub id: String,
    pub canonical_title: Option<String>,
    pub platform: Option<String>,
    pub beaten: Option<bool>,
    pub genre: Option<String>,
    pub playtime_minutes: Option<i64>,
    pub last_played_at: Option<String>,
    pub queue_is_playing: Option<bool>,
    pub queue_started_at: Option<String>,
    pub queue_finished_at: Option<String>,
    pub cover_url: Option<String>,
    pub release_year: Option<i64>,

    // license updates
    pub license_type: Option<String>,
    pub license_source: Option<String>,
}

#[tauri::command]
pub async fn update_game(db: tauri::State<'_, Db>, changes: GameUpdate) -> Result<(), String> {
    let game = fetch_game_audit_record(db.inner(), &changes.id)
        .await
        .map_err(|e| e.to_string())?;
    let change_summary = join_log_parts(summarize_game_update(&changes));

    if let Some(v) = changes.canonical_title.as_ref() {
        sqlx::query(
            "UPDATE games SET canonical_title = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
        )
        .bind(v)
        .bind(&changes.id)
        .execute(db.inner())
        .await
        .map_err(|e| e.to_string())?;
    }
    if let Some(v) = changes.platform.as_ref() {
        sqlx::query("UPDATE games SET platform = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(v)
            .bind(&changes.id)
            .execute(db.inner())
            .await
            .map_err(|e| e.to_string())?;
    }
    if let Some(v) = changes.beaten {
        sqlx::query("UPDATE games SET beaten = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(if v { 1_i64 } else { 0_i64 })
            .bind(&changes.id)
            .execute(db.inner())
            .await
            .map_err(|e| e.to_string())?;
    }
    if let Some(v) = changes.genre.as_ref() {
        sqlx::query("UPDATE games SET genre = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(v)
            .bind(&changes.id)
            .execute(db.inner())
            .await
            .map_err(|e| e.to_string())?;
    }
    if let Some(v) = changes.playtime_minutes {
        sqlx::query(
            "UPDATE games SET playtime_minutes = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
        )
        .bind(v)
        .bind(&changes.id)
        .execute(db.inner())
        .await
        .map_err(|e| e.to_string())?;
    }
    if let Some(v_raw) = changes.last_played_at.as_deref() {
        let normalized = normalize_sqlite_datetime_to_iso(Some(v_raw));
        sqlx::query(
            "UPDATE games SET last_played_at = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
        )
        .bind(normalized)
        .bind(&changes.id)
        .execute(db.inner())
        .await
        .map_err(|e| e.to_string())?;
    }
    if let Some(v) = changes.queue_is_playing {
        if v {
            let started_at = today_local_midnight_iso();
            sqlx::query(
                "UPDATE games SET queue_is_playing = 1, queue_started_at = COALESCE(queue_started_at, ?), updated_at = CURRENT_TIMESTAMP WHERE id = ?",
            )
            .bind(started_at)
            .bind(&changes.id)
            .execute(db.inner())
            .await
            .map_err(|e| e.to_string())?;
        } else {
            sqlx::query("UPDATE games SET queue_is_playing = 0, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
                .bind(&changes.id).execute(db.inner()).await.map_err(|e| e.to_string())?;
        }
    }
    if let Some(v_raw) = changes.queue_started_at.as_deref() {
        let normalized = normalize_sqlite_datetime_to_iso(Some(v_raw));
        sqlx::query(
            "UPDATE games SET queue_started_at = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
        )
        .bind(normalized)
        .bind(&changes.id)
        .execute(db.inner())
        .await
        .map_err(|e| e.to_string())?;
    }
    if let Some(v_raw) = changes.queue_finished_at.as_deref() {
        let normalized = normalize_sqlite_datetime_to_iso(Some(v_raw));
        sqlx::query(
            "UPDATE games SET queue_finished_at = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
        )
        .bind(normalized)
        .bind(&changes.id)
        .execute(db.inner())
        .await
        .map_err(|e| e.to_string())?;
    }
    if let Some(v) = changes.cover_url.as_ref() {
        sqlx::query("UPDATE games SET cover_url = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(v)
            .bind(&changes.id)
            .execute(db.inner())
            .await
            .map_err(|e| e.to_string())?;
    }
    if let Some(v) = changes.release_year {
        sqlx::query(
            "UPDATE games SET release_year = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
        )
        .bind(v)
        .bind(&changes.id)
        .execute(db.inner())
        .await
        .map_err(|e| e.to_string())?;
    }
    if let Some(v_raw) = changes.license_type.as_deref() {
        let v = normalize_license_type(Some(v_raw))?;
        if v == "subscription" {
            sqlx::query(
                "UPDATE games SET license_type = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
            )
            .bind(v)
            .bind(&changes.id)
            .execute(db.inner())
            .await
            .map_err(|e| e.to_string())?;
        } else {
            // Prevent stale labels (e.g., "PC Game Pass") when switching away from subscription.
            sqlx::query("UPDATE games SET license_type = ?, license_source = NULL, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
                .bind(v).bind(&changes.id).execute(db.inner()).await.map_err(|e| e.to_string())?;
        }
    }
    if let Some(v) = changes.license_source.as_deref() {
        let normalized = normalize_optional_text(Some(v));
        sqlx::query(
            "UPDATE games SET license_source = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
        )
        .bind(normalized)
        .bind(&changes.id)
        .execute(db.inner())
        .await
        .map_err(|e| e.to_string())?;
    }

    logging::log_db_transaction(format!(
        "Game updated {} fields={}",
        format_game_audit_record(game.as_ref(), &changes.id),
        change_summary
    ));
    Ok(())
}

#[derive(Serialize)]
pub struct FullGame {
    pub id: String,
    pub title: String,
    pub platform: String,
    pub beaten: bool,
    pub genre: Option<String>,
    pub cover_url: Option<String>,
    pub release_year: Option<i64>,
    pub playtime_minutes: Option<i64>,
    pub last_played_at: Option<String>,
    pub queue_position: Option<i64>,
    pub queue_is_playing: bool,
    pub queue_started_at: Option<String>,
    pub queue_finished_at: Option<String>,

    pub igdb_id: Option<i64>,
    pub igdb_url: Option<String>,
    pub summary: Option<String>,
    pub storyline: Option<String>,
    pub agg_rating: Option<f64>,
    pub user_rating: Option<f64>,
    pub ttb_main: Option<i64>,
    pub ttb_extra: Option<i64>,
    pub ttb_complete: Option<i64>,
    pub ttb_count: Option<i64>,
    pub hltb_id: Option<i64>,
    pub hltb_title: Option<String>,
    pub hltb_url: Option<String>,
    pub hltb_main: Option<i64>,
    pub hltb_extra: Option<i64>,
    pub hltb_complete: Option<i64>,
    pub hltb_all: Option<i64>,
    pub hltb_main_count: Option<i64>,
    pub hltb_extra_count: Option<i64>,
    pub hltb_complete_count: Option<i64>,
    pub hltb_all_count: Option<i64>,
    pub hltb_platforms: Option<String>,
    pub hltb_genres: Option<String>,
    pub hltb_summary: Option<String>,
    pub hltb_match_method: Option<String>,
    pub notes: Option<String>,
    pub review_rating: Option<i64>,
    pub review_text: Option<String>,
}

#[tauri::command]
pub async fn get_game_details(db: tauri::State<'_, Db>, id: String) -> Result<FullGame, String> {
    let r = sqlx::query(
        r#"
        SELECT id, canonical_title, platform, beaten, genre, cover_url,
               CAST(release_year AS INTEGER) AS release_year,
               playtime_minutes, last_played_at,
               queue_position, queue_is_playing, queue_started_at, queue_finished_at,
               igdb_id, igdb_url, summary, storyline, agg_rating, user_rating,
               ttb_main, ttb_extra, ttb_complete, ttb_count,
               hltb_id, hltb_title, hltb_url,
               hltb_main, hltb_extra, hltb_complete, hltb_all,
               hltb_main_count, hltb_extra_count, hltb_complete_count, hltb_all_count,
               hltb_platforms, hltb_genres, hltb_summary, hltb_match_method,
               notes,
               review_rating, review_text
        FROM games WHERE id = ?
        "#,
    )
    .bind(&id)
    .fetch_one(db.inner())
    .await
    .map_err(|e| e.to_string())?;

    Ok(FullGame {
        id: r.get::<String, _>(0),
        title: r.get::<String, _>(1),
        platform: r.get::<String, _>(2),
        beaten: r.get::<Option<i64>, _>(3).unwrap_or(0) != 0,
        genre: r.get(4),
        cover_url: r.get(5),
        release_year: r.get(6),
        playtime_minutes: r.get(7),
        last_played_at: r.get(8),
        queue_position: r.get(9),
        queue_is_playing: r.get::<Option<i64>, _>(10).unwrap_or(0) != 0,
        queue_started_at: r.get(11),
        queue_finished_at: r.get(12),

        igdb_id: r.get(13),
        igdb_url: r.get(14),
        summary: r.get(15),
        storyline: r.get(16),
        agg_rating: r.get(17),
        user_rating: r.get(18),
        ttb_main: r.get(19),
        ttb_extra: r.get(20),
        ttb_complete: r.get(21),
        ttb_count: r.get(22),
        hltb_id: r.get(23),
        hltb_title: r.get(24),
        hltb_url: r.get(25),
        hltb_main: r.get(26),
        hltb_extra: r.get(27),
        hltb_complete: r.get(28),
        hltb_all: r.get(29),
        hltb_main_count: r.get(30),
        hltb_extra_count: r.get(31),
        hltb_complete_count: r.get(32),
        hltb_all_count: r.get(33),
        hltb_platforms: r.get(34),
        hltb_genres: r.get(35),
        hltb_summary: r.get(36),
        hltb_match_method: r.get(37),
        notes: r.get(38),
        review_rating: r.get(39),
        review_text: r.get(40),
    })
}

#[tauri::command]
pub async fn add_game_to_queue(db: tauri::State<'_, Db>, id: String) -> Result<(), String> {
    let mut tx = db.begin().await.map_err(|e| e.to_string())?;

    let row = sqlx::query(
        "SELECT canonical_title, platform, queue_position, last_played_at FROM games WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| format!("Game not found: {}", id))?;

    let title: String = row.get(0);
    let platform: String = row.get(1);
    let current_position: Option<i64> = row.get(2);
    if current_position.is_some() {
        tx.commit().await.map_err(|e| e.to_string())?;
        logging::log_db_transaction(format!(
            "Queue add skipped {} reason=already_queued",
            format_game_identity(&title, &platform, &id)
        ));
        return Ok(());
    }

    let last_played_at: Option<String> = row.get(3);
    let next_position: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(queue_position), 0) + 1 FROM games WHERE queue_position IS NOT NULL",
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;

    let started_at = normalize_sqlite_datetime_to_iso(last_played_at.as_deref());
    let started_at_for_log = started_at.clone();

    sqlx::query(
        "UPDATE games SET queue_position = ?, queue_is_playing = 0, queue_started_at = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
    )
    .bind(next_position)
    .bind(started_at)
    .bind(&id)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;

    tx.commit().await.map_err(|e| e.to_string())?;
    logging::log_db_transaction(format!(
        "Game added to queue {} queue_position={} queue_started_at={}",
        format_game_identity(&title, &platform, &id),
        next_position,
        format_optional_text_for_log(started_at_for_log.as_deref(), 32)
    ));
    Ok(())
}

#[tauri::command]
pub async fn remove_game_from_queue(db: tauri::State<'_, Db>, id: String) -> Result<(), String> {
    let mut tx = db.begin().await.map_err(|e| e.to_string())?;

    let row =
        sqlx::query("SELECT canonical_title, platform, queue_position FROM games WHERE id = ?")
            .bind(&id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Game not found: {}", id))?;

    let title: String = row.get(0);
    let platform: String = row.get(1);
    let removed_position: Option<i64> = row.get(2);

    let Some(removed_position) = removed_position else {
        tx.commit().await.map_err(|e| e.to_string())?;
        logging::log_db_transaction(format!(
            "Queue remove skipped {} reason=not_queued",
            format_game_identity(&title, &platform, &id)
        ));
        return Ok(());
    };

    sqlx::query(
        "UPDATE games SET queue_position = NULL, queue_is_playing = 0, queue_started_at = NULL, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
    )
    .bind(&id)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;

    sqlx::query(
        "UPDATE games SET queue_position = queue_position - 1, updated_at = CURRENT_TIMESTAMP WHERE queue_position > ?",
    )
    .bind(removed_position)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;

    tx.commit().await.map_err(|e| e.to_string())?;
    logging::log_db_transaction(format!(
        "Game removed from queue {} previous_queue_position={}",
        format_game_identity(&title, &platform, &id),
        removed_position
    ));
    Ok(())
}

#[tauri::command]
pub async fn reorder_queue(db: tauri::State<'_, Db>, ids: Vec<String>) -> Result<(), String> {
    let mut tx = db.begin().await.map_err(|e| e.to_string())?;

    for (idx, id) in ids.iter().enumerate() {
        sqlx::query(
            "UPDATE games SET queue_position = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
        )
        .bind((idx as i64) + 1)
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }

    tx.commit().await.map_err(|e| e.to_string())?;
    logging::log_db_transaction(format!("Queue reordered items={}", ids.len()));
    Ok(())
}

#[tauri::command]
pub async fn add_queue_playtime(
    db: tauri::State<'_, Db>,
    id: String,
    minutes_to_add: i64,
) -> Result<i64, String> {
    if minutes_to_add <= 0 {
        return Err("minutes_to_add must be greater than zero.".to_string());
    }

    let game = fetch_game_audit_record(db.inner(), &id)
        .await
        .map_err(|e| e.to_string())?;

    sqlx::query(
        "UPDATE games SET playtime_minutes = COALESCE(playtime_minutes, 0) + ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
    )
    .bind(minutes_to_add)
    .bind(&id)
    .execute(db.inner())
    .await
    .map_err(|e| e.to_string())?;

    let total: i64 =
        sqlx::query_scalar("SELECT COALESCE(playtime_minutes, 0) FROM games WHERE id = ?")
            .bind(&id)
            .fetch_one(db.inner())
            .await
            .map_err(|e| e.to_string())?;

    logging::log_db_transaction(format!(
        "Queue playtime added {} minutes_added={} new_total_minutes={}",
        format_game_audit_record(game.as_ref(), &id),
        minutes_to_add,
        total
    ));

    Ok(total)
}

// ========================= CSV backups =========================

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub struct LibraryBackupRow {
    pub title: String,
    pub platform: String,
    pub beaten: bool,
    pub genre: Option<String>,
    pub playtime_minutes: Option<i64>,
    pub last_played_at: Option<String>,
    pub release_year: Option<i64>,
    pub cover_url: Option<String>,
    pub license_type: Option<String>,
    pub license_source: Option<String>,
    pub hidden: bool,
    pub notes: Option<String>,
    pub review_rating: Option<i64>,
    pub review_text: Option<String>,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct LibraryBackupImportRow {
    pub title: String,
    pub platform: Option<String>,
    pub beaten: Option<bool>,
    pub genre: Option<String>,
    pub playtime_minutes: Option<i64>,
    pub last_played_at: Option<String>,
    pub release_year: Option<i64>,
    pub cover_url: Option<String>,
    pub license_type: Option<String>,
    pub license_source: Option<String>,
    pub hidden: Option<bool>,
    pub notes: Option<String>,
    pub review_rating: Option<i64>,
    pub review_text: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub struct LibraryBackupImportResult {
    pub created: u32,
    pub updated: u32,
    pub skipped: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub struct QueueBackupRow {
    pub queue_position: i64,
    pub title: String,
    pub platform: String,
    pub queue_is_playing: bool,
    pub queue_started_at: Option<String>,
    pub queue_finished_at: Option<String>,
    pub playtime_minutes: Option<i64>,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct QueueBackupImportRow {
    pub queue_position: Option<i64>,
    pub title: String,
    pub platform: Option<String>,
    pub queue_is_playing: Option<bool>,
    pub queue_started_at: Option<String>,
    pub queue_finished_at: Option<String>,
    pub playtime_minutes: Option<i64>,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub struct QueueBackupImportResult {
    pub restored: u32,
    pub created: u32,
    pub skipped: u32,
}

#[tauri::command]
pub async fn default_backup_export_path(filename: String) -> Result<String, String> {
    let file_name = normalize_optional_text(Some(&filename))
        .ok_or_else(|| "A backup filename is required.".to_string())?;

    let base = dirs::download_dir()
        .or_else(dirs::document_dir)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));

    Ok(base.join(file_name).to_string_lossy().to_string())
}

#[tauri::command]
pub async fn save_backup_file(path: String, contents: String) -> Result<String, String> {
    let path = normalize_optional_text(Some(&path))
        .ok_or_else(|| "A save path is required.".to_string())?;
    let output = PathBuf::from(&path);
    let content_bytes = contents.as_bytes().len();

    if output.file_name().is_none() {
        return Err("Please include a file name in the save path.".to_string());
    }

    if let Some(parent) = output.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
    }

    std::fs::write(&output, contents).map_err(|e| e.to_string())?;
    logging::log_backup(format!(
        "Backup file saved path={} bytes={}",
        quote_for_log(&output.to_string_lossy(), 220),
        content_bytes
    ));
    Ok(output.to_string_lossy().to_string())
}

#[cfg(test)]
mod backup_tests {
    use super::*;

    #[test]
    fn library_backup_rows_serialize_with_snake_case_keys() {
        let row = LibraryBackupRow {
            title: "Test Game".to_string(),
            platform: "Steam".to_string(),
            beaten: true,
            genre: Some("RPG".to_string()),
            playtime_minutes: Some(123),
            last_played_at: Some("2026-03-20T00:00:00Z".to_string()),
            release_year: Some(2024),
            cover_url: Some("https://example.com/cover.jpg".to_string()),
            license_type: Some("owned".to_string()),
            license_source: Some("Steam".to_string()),
            hidden: false,
            notes: Some("notes".to_string()),
            review_rating: Some(4),
            review_text: Some("review".to_string()),
        };

        let json = serde_json::to_value(row).expect("library backup row should serialize");
        assert!(json.get("playtime_minutes").is_some());
        assert!(json.get("last_played_at").is_some());
        assert!(json.get("release_year").is_some());
        assert!(json.get("cover_url").is_some());
        assert!(json.get("license_type").is_some());
        assert!(json.get("review_text").is_some());
        assert!(json.get("playtimeMinutes").is_none());
        assert!(json.get("lastPlayedAt").is_none());
    }

    #[test]
    fn queue_backup_rows_serialize_with_snake_case_keys() {
        let row = QueueBackupRow {
            queue_position: 2,
            title: "Queued Game".to_string(),
            platform: "Xbox".to_string(),
            queue_is_playing: true,
            queue_started_at: Some("2026-03-18T00:00:00Z".to_string()),
            queue_finished_at: Some("2026-03-19T00:00:00Z".to_string()),
            playtime_minutes: Some(85),
        };

        let json = serde_json::to_value(row).expect("queue backup row should serialize");
        assert!(json.get("queue_position").is_some());
        assert!(json.get("queue_is_playing").is_some());
        assert!(json.get("queue_started_at").is_some());
        assert!(json.get("queue_finished_at").is_some());
        assert!(json.get("playtime_minutes").is_some());
        assert!(json.get("queuePosition").is_none());
        assert!(json.get("queueIsPlaying").is_none());
    }
}

fn normalize_required_backup_title(raw: &str) -> Option<String> {
    normalize_optional_text(Some(raw))
}

fn normalize_optional_platform_value(raw: Option<&str>) -> Option<String> {
    normalize_optional_text(raw)
}

async fn find_existing_game_for_backup(
    conn: &mut SqliteConnection,
    title: &str,
    platform: Option<&str>,
) -> Result<Option<String>, String> {
    if let Some(platform) =
        platform.and_then(|value| normalize_optional_platform_value(Some(value)))
    {
        let existing = sqlx::query_scalar::<sqlx::Sqlite, String>(
            r#"
            SELECT id
            FROM games
            WHERE LOWER(TRIM(canonical_title)) = LOWER(TRIM(?))
              AND LOWER(TRIM(platform)) = LOWER(TRIM(?))
            ORDER BY updated_at DESC, id DESC
            LIMIT 1
            "#,
        )
        .bind(title)
        .bind(platform)
        .fetch_optional(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;

        return Ok(existing);
    }

    let matches = sqlx::query_scalar::<sqlx::Sqlite, String>(
        r#"
        SELECT id
        FROM games
        WHERE LOWER(TRIM(canonical_title)) = LOWER(TRIM(?))
        ORDER BY updated_at DESC, id DESC
        LIMIT 2
        "#,
    )
    .bind(title)
    .fetch_all(&mut *conn)
    .await
    .map_err(|e| e.to_string())?;

    if matches.len() == 1 {
        Ok(matches.into_iter().next())
    } else {
        Ok(None)
    }
}

async fn insert_backup_game_stub(
    conn: &mut SqliteConnection,
    title: &str,
    platform: &str,
    owned_source: &str,
    beaten: bool,
) -> Result<String, String> {
    let id = uuid::Uuid::new_v4().to_string();

    sqlx::query(
        r#"
        INSERT INTO games (
          id, canonical_title, alt_titles, platform, owned_source, beaten, license_type
        )
        VALUES (?, ?, '[]', ?, ?, ?, 'owned')
        "#,
    )
    .bind(&id)
    .bind(title)
    .bind(platform)
    .bind(owned_source)
    .bind(if beaten { 1_i64 } else { 0_i64 })
    .execute(&mut *conn)
    .await
    .map_err(|e| e.to_string())?;

    Ok(id)
}

async fn apply_library_backup_fields(
    conn: &mut SqliteConnection,
    id: &str,
    row: &LibraryBackupImportRow,
    allow_platform_update: bool,
) -> Result<(), String> {
    let title = normalize_required_backup_title(&row.title)
        .ok_or_else(|| "Library backup row is missing a title.".to_string())?;

    sqlx::query(
        "UPDATE games SET canonical_title = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
    )
    .bind(&title)
    .bind(id)
    .execute(&mut *conn)
    .await
    .map_err(|e| e.to_string())?;

    if allow_platform_update {
        if let Some(platform) = row
            .platform
            .as_deref()
            .and_then(|value| normalize_optional_platform_value(Some(value)))
        {
            sqlx::query(
                "UPDATE games SET platform = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
            )
            .bind(platform)
            .bind(id)
            .execute(&mut *conn)
            .await
            .map_err(|e| e.to_string())?;
        }
    }

    if let Some(beaten) = row.beaten {
        sqlx::query("UPDATE games SET beaten = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(if beaten { 1_i64 } else { 0_i64 })
            .bind(id)
            .execute(&mut *conn)
            .await
            .map_err(|e| e.to_string())?;
    }

    if let Some(genre) = row
        .genre
        .as_deref()
        .and_then(|value| normalize_optional_text(Some(value)))
    {
        sqlx::query("UPDATE games SET genre = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(genre)
            .bind(id)
            .execute(&mut *conn)
            .await
            .map_err(|e| e.to_string())?;
    }

    if let Some(playtime_minutes) = row.playtime_minutes.filter(|value| *value >= 0) {
        sqlx::query(
            "UPDATE games SET playtime_minutes = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
        )
        .bind(playtime_minutes)
        .bind(id)
        .execute(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;
    }

    if let Some(last_played_at) = row
        .last_played_at
        .as_deref()
        .and_then(|value| normalize_optional_text(Some(value)))
    {
        sqlx::query(
            "UPDATE games SET last_played_at = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
        )
        .bind(normalize_sqlite_datetime_to_iso(Some(&last_played_at)))
        .bind(id)
        .execute(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;
    }

    if let Some(release_year) = row.release_year.filter(|value| *value > 0) {
        sqlx::query(
            "UPDATE games SET release_year = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
        )
        .bind(release_year)
        .bind(id)
        .execute(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;
    }

    if let Some(cover_url) = row
        .cover_url
        .as_deref()
        .and_then(|value| normalize_optional_text(Some(value)))
    {
        sqlx::query("UPDATE games SET cover_url = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(cover_url)
            .bind(id)
            .execute(&mut *conn)
            .await
            .map_err(|e| e.to_string())?;
    }

    if let Some(license_type_raw) = row
        .license_type
        .as_deref()
        .and_then(|value| normalize_optional_text(Some(value)))
    {
        let license_type = normalize_license_type(Some(license_type_raw.as_str()))?;
        let license_source = normalize_license_source(license_type, row.license_source.as_deref());

        if license_type == "subscription" {
            sqlx::query(
                "UPDATE games SET license_type = ?, license_source = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
            )
            .bind(license_type)
            .bind(license_source)
            .bind(id)
            .execute(&mut *conn)
            .await
            .map_err(|e| e.to_string())?;
        } else {
            sqlx::query(
                "UPDATE games SET license_type = ?, license_source = NULL, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
            )
            .bind(license_type)
            .bind(id)
            .execute(&mut *conn)
            .await
            .map_err(|e| e.to_string())?;
        }
    } else if let Some(license_source) = row
        .license_source
        .as_deref()
        .and_then(|value| normalize_optional_text(Some(value)))
    {
        sqlx::query(
            "UPDATE games SET license_source = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
        )
        .bind(license_source)
        .bind(id)
        .execute(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;
    }

    if let Some(hidden) = row.hidden {
        if hidden {
            sqlx::query(
                "UPDATE games SET hidden_at = COALESCE(hidden_at, CURRENT_TIMESTAMP), updated_at = CURRENT_TIMESTAMP WHERE id = ?",
            )
            .bind(id)
            .execute(&mut *conn)
            .await
            .map_err(|e| e.to_string())?;
        } else {
            sqlx::query(
                "UPDATE games SET hidden_at = NULL, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
            )
            .bind(id)
            .execute(&mut *conn)
            .await
            .map_err(|e| e.to_string())?;
        }
    }

    if let Some(notes) = row
        .notes
        .as_deref()
        .and_then(|value| normalize_optional_text(Some(value)))
    {
        sqlx::query("UPDATE games SET notes = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(notes)
            .bind(id)
            .execute(&mut *conn)
            .await
            .map_err(|e| e.to_string())?;
    }

    if let Some(review_rating) = row.review_rating.filter(|value| (0..=5).contains(value)) {
        sqlx::query(
            "UPDATE games SET review_rating = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
        )
        .bind(review_rating)
        .bind(id)
        .execute(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;
    }

    if let Some(review_text) = row
        .review_text
        .as_deref()
        .and_then(|value| normalize_optional_text(Some(value)))
    {
        sqlx::query(
            "UPDATE games SET review_text = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
        )
        .bind(review_text)
        .bind(id)
        .execute(&mut *conn)
        .await
        .map_err(|e| e.to_string())?;
    }

    Ok(())
}

async fn upsert_library_backup_row(
    conn: &mut SqliteConnection,
    row: &LibraryBackupImportRow,
    owned_source: &str,
) -> Result<bool, String> {
    let title = normalize_required_backup_title(&row.title)
        .ok_or_else(|| "Library backup row is missing a title.".to_string())?;
    let platform = row
        .platform
        .as_deref()
        .and_then(|value| normalize_optional_platform_value(Some(value)));

    if let Some(existing_id) =
        find_existing_game_for_backup(&mut *conn, &title, platform.as_deref()).await?
    {
        apply_library_backup_fields(&mut *conn, &existing_id, row, false).await?;
        return Ok(false);
    }

    let created_id = insert_backup_game_stub(
        &mut *conn,
        &title,
        platform.as_deref().unwrap_or("Manual"),
        owned_source,
        row.beaten.unwrap_or(false),
    )
    .await?;

    apply_library_backup_fields(&mut *conn, &created_id, row, true).await?;

    Ok(true)
}

#[tauri::command]
pub async fn export_library_backup(
    db: tauri::State<'_, Db>,
) -> Result<Vec<LibraryBackupRow>, String> {
    let rows = sqlx::query(
        r#"
        SELECT
          canonical_title,
          platform,
          beaten,
          genre,
          playtime_minutes,
          last_played_at,
          CAST(release_year AS INTEGER) AS release_year,
          cover_url,
          license_type,
          license_source,
          hidden_at,
          notes,
          review_rating,
          review_text
        FROM games
        ORDER BY canonical_title, platform, id
        "#,
    )
    .fetch_all(db.inner())
    .await
    .map_err(|e| e.to_string())?;

    let export_rows: Vec<LibraryBackupRow> = rows
        .into_iter()
        .map(|row| LibraryBackupRow {
            title: row.get::<String, _>(0),
            platform: row.get::<String, _>(1),
            beaten: row.get::<Option<i64>, _>(2).unwrap_or(0) != 0,
            genre: row.get(3),
            playtime_minutes: row.get(4),
            last_played_at: row.get(5),
            release_year: row.get(6),
            cover_url: row.get(7),
            license_type: row.get(8),
            license_source: row.get(9),
            hidden: row.get::<Option<String>, _>(10).is_some(),
            notes: row.get(11),
            review_rating: row.get(12),
            review_text: row.get(13),
        })
        .collect();

    logging::log_backup(format!(
        "Library backup exported rows={}",
        export_rows.len()
    ));

    Ok(export_rows)
}

#[tauri::command]
pub async fn import_library_backup(
    db: tauri::State<'_, Db>,
    rows: Vec<LibraryBackupImportRow>,
) -> Result<LibraryBackupImportResult, String> {
    let run_id = uuid::Uuid::new_v4().to_string();
    let run_tag = run_id.get(..8).unwrap_or(run_id.as_str()).to_string();
    let valid_titles = rows
        .iter()
        .filter(|row| normalize_required_backup_title(&row.title).is_some())
        .count();

    if valid_titles == 0 {
        return Err("No valid library rows were found. A title column is required.".to_string());
    }

    logging::log_backup(format!(
        "[run:{}] library_backup_import_started rows={} valid_rows={}",
        run_tag,
        rows.len(),
        valid_titles
    ));

    let mut tx = db.begin().await.map_err(|e| e.to_string())?;
    let mut created = 0_u32;
    let mut updated = 0_u32;
    let mut skipped = 0_u32;

    for row in rows {
        if normalize_required_backup_title(&row.title).is_none() {
            skipped += 1;
            logging::log_backup(format!(
                "[run:{}] library_backup_row_skipped title={} reason=missing_title",
                run_tag,
                quote_for_log(&row.title, 96)
            ));
            continue;
        }

        match upsert_library_backup_row(&mut *tx, &row, "Library Backup").await {
            Ok(true) => created += 1,
            Ok(false) => updated += 1,
            Err(err) => {
                eprintln!(
                    "[library backup import] skipped row '{}': {}",
                    row.title, err
                );
                logging::log_backup(format!(
                    "[run:{}] library_backup_row_skipped title={} reason={}",
                    run_tag,
                    quote_for_log(&row.title, 96),
                    quote_for_log(&err, 160)
                ));
                skipped += 1;
            }
        }
    }

    tx.commit().await.map_err(|e| e.to_string())?;

    logging::log_backup(format!(
        "[run:{}] library_backup_import_finished created={} updated={} skipped={}",
        run_tag, created, updated, skipped
    ));

    Ok(LibraryBackupImportResult {
        created,
        updated,
        skipped,
    })
}

#[tauri::command]
pub async fn export_queue_backup(db: tauri::State<'_, Db>) -> Result<Vec<QueueBackupRow>, String> {
    let rows = sqlx::query(
        r#"
        SELECT
          queue_position,
          canonical_title,
          platform,
          queue_is_playing,
          queue_started_at,
          queue_finished_at,
          playtime_minutes
        FROM games
        WHERE queue_position IS NOT NULL
        ORDER BY queue_position, canonical_title, id
        "#,
    )
    .fetch_all(db.inner())
    .await
    .map_err(|e| e.to_string())?;

    let export_rows: Vec<QueueBackupRow> = rows
        .into_iter()
        .map(|row| QueueBackupRow {
            queue_position: row.get::<Option<i64>, _>(0).unwrap_or(0),
            title: row.get::<String, _>(1),
            platform: row.get::<String, _>(2),
            queue_is_playing: row.get::<Option<i64>, _>(3).unwrap_or(0) != 0,
            queue_started_at: row.get(4),
            queue_finished_at: row.get(5),
            playtime_minutes: row.get(6),
        })
        .collect();

    logging::log_backup(format!("Queue backup exported rows={}", export_rows.len()));

    Ok(export_rows)
}

#[tauri::command]
pub async fn import_queue_backup(
    db: tauri::State<'_, Db>,
    rows: Vec<QueueBackupImportRow>,
) -> Result<QueueBackupImportResult, String> {
    let run_id = uuid::Uuid::new_v4().to_string();
    let run_tag = run_id.get(..8).unwrap_or(run_id.as_str()).to_string();
    let mut indexed_rows: Vec<(usize, QueueBackupImportRow)> =
        rows.into_iter().enumerate().collect();

    let valid_titles = indexed_rows
        .iter()
        .filter(|(_, row)| normalize_required_backup_title(&row.title).is_some())
        .count();

    if valid_titles == 0 {
        return Err("No valid queue rows were found. A title column is required.".to_string());
    }

    logging::log_backup(format!(
        "[run:{}] queue_backup_import_started rows={} valid_rows={}",
        run_tag,
        indexed_rows.len(),
        valid_titles
    ));

    indexed_rows.sort_by_key(|(idx, row)| (row.queue_position.unwrap_or((*idx as i64) + 1), *idx));

    let mut tx = db.begin().await.map_err(|e| e.to_string())?;
    let mut restored = 0_u32;
    let mut created = 0_u32;
    let mut skipped = 0_u32;
    let mut seen_ids = std::collections::HashSet::<String>::new();

    sqlx::query(
        r#"
        UPDATE games
        SET queue_position = NULL,
            queue_is_playing = 0,
            queue_started_at = NULL,
            updated_at = CURRENT_TIMESTAMP
        WHERE queue_position IS NOT NULL
           OR queue_is_playing != 0
           OR queue_started_at IS NOT NULL
        "#,
    )
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;

    for (_, row) in indexed_rows {
        let Some(title) = normalize_required_backup_title(&row.title) else {
            skipped += 1;
            logging::log_backup(format!(
                "[run:{}] queue_backup_row_skipped title={} reason=missing_title",
                run_tag,
                quote_for_log(&row.title, 96)
            ));
            continue;
        };

        let platform = row
            .platform
            .as_deref()
            .and_then(|value| normalize_optional_platform_value(Some(value)));
        let playtime_row = LibraryBackupImportRow {
            title: title.clone(),
            platform: platform.clone(),
            beaten: None,
            genre: None,
            playtime_minutes: row.playtime_minutes,
            last_played_at: None,
            release_year: None,
            cover_url: None,
            license_type: None,
            license_source: None,
            hidden: None,
            notes: None,
            review_rating: None,
            review_text: None,
        };

        let game_id = match find_existing_game_for_backup(&mut *tx, &title, platform.as_deref())
            .await
        {
            Ok(Some(existing_id)) => {
                apply_library_backup_fields(&mut *tx, &existing_id, &playtime_row, false).await?;
                existing_id
            }
            Ok(None) => {
                let inserted_id = insert_backup_game_stub(
                    &mut *tx,
                    &title,
                    platform.as_deref().unwrap_or("Manual"),
                    "Queue Backup",
                    false,
                )
                .await?;
                apply_library_backup_fields(&mut *tx, &inserted_id, &playtime_row, true).await?;
                created += 1;
                inserted_id
            }
            Err(err) => {
                eprintln!("[queue backup import] skipped row '{}': {}", row.title, err);
                logging::log_backup(format!(
                    "[run:{}] queue_backup_row_skipped title={} reason={}",
                    run_tag,
                    quote_for_log(&row.title, 96),
                    quote_for_log(&err, 160)
                ));
                skipped += 1;
                continue;
            }
        };

        if !seen_ids.insert(game_id.clone()) {
            skipped += 1;
            logging::log_backup(format!(
                "[run:{}] queue_backup_row_skipped title={} reason=duplicate_target_game",
                run_tag,
                quote_for_log(&title, 96)
            ));
            continue;
        }

        let queue_is_playing = row.queue_is_playing.unwrap_or(false);
        let started_at =
            normalize_sqlite_datetime_to_iso(row.queue_started_at.as_deref()).or_else(|| {
                if queue_is_playing {
                    Some(today_local_midnight_iso())
                } else {
                    None
                }
            });
        let finished_at = normalize_sqlite_datetime_to_iso(row.queue_finished_at.as_deref());
        let next_position = (restored as i64) + 1;

        sqlx::query(
            r#"
            UPDATE games
            SET queue_position = ?,
                queue_is_playing = ?,
                queue_started_at = ?,
                queue_finished_at = ?,
                updated_at = CURRENT_TIMESTAMP
            WHERE id = ?
            "#,
        )
        .bind(next_position)
        .bind(if queue_is_playing { 1_i64 } else { 0_i64 })
        .bind(started_at)
        .bind(finished_at)
        .bind(&game_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

        restored += 1;
    }

    tx.commit().await.map_err(|e| e.to_string())?;

    logging::log_backup(format!(
        "[run:{}] queue_backup_import_finished restored={} created={} skipped={}",
        run_tag, restored, created, skipped
    ));

    Ok(QueueBackupImportResult {
        restored,
        created,
        skipped,
    })
}

// --- IGDB one-off helpers ---

#[tauri::command]
pub async fn lookup_igdb_preview(
    auth: IgdbAuth,
    title: String,
) -> Result<crate::services::igdb::IgdbEnrich, String> {
    use crate::services::igdb::{get_oauth_token, lookup_enrich_for_title};
    let token = get_oauth_token(&auth.client_id, &auth.client_secret)
        .await
        .map_err(|e| e.to_string())?;
    match lookup_enrich_for_title(&auth.client_id, &token, &title).await {
        Ok(Some(info)) => Ok(info),
        Ok(None) => Err("No IGDB match".to_string()),
        Err(e) => Err(e.to_string()),
    }
}

#[derive(Deserialize)]
pub struct IgdbApplyPayload {
    pub id: String,
    pub igdb_id: Option<i64>,
    pub igdb_url: Option<String>,
    pub genres: Option<Vec<String>>,
    pub cover_url: Option<String>,
    pub release_year: Option<i64>,
    pub summary: Option<String>,
    pub storyline: Option<String>,
    pub agg_rating: Option<f64>,
    pub user_rating: Option<f64>,
    pub ttb_main: Option<i64>,
    pub ttb_extra: Option<i64>,
    pub ttb_complete: Option<i64>,
    pub ttb_count: Option<i64>,
}

#[tauri::command]
pub async fn apply_igdb_details(
    db: tauri::State<'_, Db>,
    p: IgdbApplyPayload,
) -> Result<(), String> {
    let game = fetch_game_audit_record(db.inner(), &p.id)
        .await
        .map_err(|e| e.to_string())?;
    let change_summary = join_log_parts(summarize_igdb_apply_payload(&p));

    let mut qb = QueryBuilder::<sqlx::Sqlite>::new(
        "UPDATE games SET updated_at = CURRENT_TIMESTAMP, enriched_at = CURRENT_TIMESTAMP",
    );

    if let Some(v) = p.igdb_id {
        qb.push(", igdb_id = ").push_bind(v);
    }
    if let Some(v) = p.igdb_url.filter(|s| !s.trim().is_empty()) {
        qb.push(", igdb_url = ").push_bind(v);
    }
    if let Some(gs) = p.genres.as_ref().filter(|g| !g.is_empty()) {
        qb.push(", genre = ").push_bind(gs.join(", "));
    }
    if let Some(v) = p.cover_url.filter(|s| !s.is_empty()) {
        qb.push(", cover_url = ").push_bind(v);
    }
    if let Some(v) = p.release_year {
        qb.push(", release_year = ").push_bind(v);
    }
    if let Some(v) = p.summary.filter(|s| !s.trim().is_empty()) {
        qb.push(", summary = ").push_bind(v);
    }
    if let Some(v) = p.storyline.filter(|s| !s.trim().is_empty()) {
        qb.push(", storyline = ").push_bind(v);
    }
    if let Some(v) = p.agg_rating {
        qb.push(", agg_rating = ").push_bind(v);
    }
    if let Some(v) = p.user_rating {
        qb.push(", user_rating = ").push_bind(v);
    }
    if let Some(v) = p.ttb_main {
        qb.push(", ttb_main = ").push_bind(v);
    }
    if let Some(v) = p.ttb_extra {
        qb.push(", ttb_extra = ").push_bind(v);
    }
    if let Some(v) = p.ttb_complete {
        qb.push(", ttb_complete = ").push_bind(v);
    }
    if let Some(v) = p.ttb_count {
        qb.push(", ttb_count = ").push_bind(v);
    }

    qb.push(" WHERE id = ").push_bind(&p.id);

    qb.build()
        .execute(db.inner())
        .await
        .map_err(|e| e.to_string())?;

    logging::log_enrichment(format!(
        "manual_igdb_enrichment_applied {} fields={}",
        format_game_audit_record(game.as_ref(), &p.id),
        change_summary
    ));
    logging::log_db_transaction(format!(
        "Manual IGDB enrichment applied {} fields={}",
        format_game_audit_record(game.as_ref(), &p.id),
        change_summary
    ));

    Ok(())
}

#[tauri::command]
pub async fn lookup_hltb_preview(
    title: String,
    platform: Option<String>,
) -> Result<crate::services::hltb::HltbEnrich, String> {
    match crate::services::hltb::lookup_best_for_title(&title, platform.as_deref()).await {
        Ok(Some(info)) => Ok(info),
        Ok(None) => Err("No confident HLTB match".to_string()),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub async fn lookup_hltb_preview_by_id(
    hltb_id: i64,
) -> Result<crate::services::hltb::HltbEnrich, String> {
    match crate::services::hltb::lookup_by_id(hltb_id).await {
        Ok(Some(info)) => Ok(info),
        Ok(None) => Err("No HLTB match".to_string()),
        Err(e) => Err(e.to_string()),
    }
}

#[derive(Deserialize)]
pub struct HltbApplyPayload {
    pub id: String,
    pub hltb_id: Option<i64>,
    pub title: Option<String>,
    pub profile_url: Option<String>,
    pub hltb_main: Option<i64>,
    pub hltb_extra: Option<i64>,
    pub hltb_complete: Option<i64>,
    pub hltb_all: Option<i64>,
    pub hltb_main_count: Option<i64>,
    pub hltb_extra_count: Option<i64>,
    pub hltb_complete_count: Option<i64>,
    pub hltb_all_count: Option<i64>,
    pub platforms: Option<String>,
    pub genres: Option<String>,
    pub summary: Option<String>,
    pub match_method: Option<String>,
}

#[tauri::command]
pub async fn apply_hltb_details(
    db: tauri::State<'_, Db>,
    p: HltbApplyPayload,
) -> Result<(), String> {
    let game = fetch_game_audit_record(db.inner(), &p.id)
        .await
        .map_err(|e| e.to_string())?;
    let change_summary = join_log_parts(summarize_hltb_apply_payload(&p));

    let mut qb = QueryBuilder::<sqlx::Sqlite>::new(
        "UPDATE games
         SET updated_at = CURRENT_TIMESTAMP,
             enriched_at = CURRENT_TIMESTAMP,
             hltb_updated_at = CURRENT_TIMESTAMP",
    );

    if let Some(v) = p.hltb_id {
        qb.push(", hltb_id = ").push_bind(v);
    }
    if let Some(v) = p.title.filter(|s| !s.trim().is_empty()) {
        qb.push(", hltb_title = ").push_bind(v);
    }
    if let Some(v) = p.profile_url.filter(|s| !s.trim().is_empty()) {
        qb.push(", hltb_url = ").push_bind(v);
    }
    if let Some(v) = p.hltb_main {
        qb.push(", hltb_main = ").push_bind(v);
    }
    if let Some(v) = p.hltb_extra {
        qb.push(", hltb_extra = ").push_bind(v);
    }
    if let Some(v) = p.hltb_complete {
        qb.push(", hltb_complete = ").push_bind(v);
    }
    if let Some(v) = p.hltb_all {
        qb.push(", hltb_all = ").push_bind(v);
    }
    if let Some(v) = p.hltb_main_count {
        qb.push(", hltb_main_count = ").push_bind(v);
    }
    if let Some(v) = p.hltb_extra_count {
        qb.push(", hltb_extra_count = ").push_bind(v);
    }
    if let Some(v) = p.hltb_complete_count {
        qb.push(", hltb_complete_count = ").push_bind(v);
    }
    if let Some(v) = p.hltb_all_count {
        qb.push(", hltb_all_count = ").push_bind(v);
    }
    if let Some(v) = p.platforms.filter(|s| !s.trim().is_empty()) {
        qb.push(", hltb_platforms = ").push_bind(v);
    }
    if let Some(v) = p.genres.filter(|s| !s.trim().is_empty()) {
        qb.push(", hltb_genres = ").push_bind(v);
    }
    if let Some(v) = p.summary.filter(|s| !s.trim().is_empty()) {
        qb.push(", hltb_summary = ").push_bind(v);
    }
    if let Some(v) = p.match_method.filter(|s| !s.trim().is_empty()) {
        qb.push(", hltb_match_method = ").push_bind(v);
    }

    qb.push(" WHERE id = ").push_bind(&p.id);

    qb.build()
        .execute(db.inner())
        .await
        .map_err(|e| e.to_string())?;

    logging::log_enrichment(format!(
        "manual_hltb_enrichment_applied {} fields={}",
        format_game_audit_record(game.as_ref(), &p.id),
        change_summary
    ));
    logging::log_db_transaction(format!(
        "Manual HLTB enrichment applied {} fields={}",
        format_game_audit_record(game.as_ref(), &p.id),
        change_summary
    ));

    Ok(())
}

#[tauri::command]
pub async fn save_game_notes(
    db: tauri::State<'_, Db>,
    id: String,
    notes: String,
) -> Result<(), String> {
    let game = fetch_game_audit_record(db.inner(), &id)
        .await
        .map_err(|e| e.to_string())?;
    let note_chars = notes.chars().count();

    sqlx::query("UPDATE games SET notes = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
        .bind(notes)
        .bind(&id)
        .execute(db.inner())
        .await
        .map_err(|e| e.to_string())?;

    logging::log_db_transaction(format!(
        "Game notes updated {} note_chars={}",
        format_game_audit_record(game.as_ref(), &id),
        note_chars
    ));
    Ok(())
}

#[tauri::command]
pub async fn save_game_review(
    db: tauri::State<'_, Db>,
    id: String,
    rating: Option<i64>,
    review: Option<String>,
) -> Result<(), String> {
    if let Some(value) = rating {
        if !(0..=5).contains(&value) {
            return Err("Review rating must be between 0 and 5.".to_string());
        }
    }

    let review = normalize_optional_text(review.as_deref());
    let game = fetch_game_audit_record(db.inner(), &id)
        .await
        .map_err(|e| e.to_string())?;
    let review_chars = review
        .as_ref()
        .map(|text| text.chars().count())
        .unwrap_or(0);

    sqlx::query("UPDATE games SET review_rating = ?, review_text = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
        .bind(rating)
        .bind(review)
        .bind(&id)
        .execute(db.inner())
        .await
        .map_err(|e| e.to_string())?;

    logging::log_db_transaction(format!(
        "Game review updated {} rating={} review_chars={}",
        format_game_audit_record(game.as_ref(), &id),
        rating
            .map(|value| value.to_string())
            .unwrap_or_else(|| "null".to_string()),
        review_chars
    ));
    Ok(())
}

#[derive(serde::Serialize)]
pub struct IgdbCandidateDto {
    pub igdb_id: i64,
    pub title: String,
    pub cover_url: Option<String>,
    pub release_year: Option<i32>,
}

#[tauri::command]
pub async fn lookup_igdb_candidates(
    auth: IgdbAuth,
    title: String,
) -> Result<Vec<IgdbCandidateDto>, String> {
    use crate::services::igdb::{get_oauth_token, search_candidates};
    let token = get_oauth_token(&auth.client_id, &auth.client_secret)
        .await
        .map_err(|e| e.to_string())?;
    let list = search_candidates(&auth.client_id, &token, &title)
        .await
        .map_err(|e| e.to_string())?;
    Ok(list
        .into_iter()
        .map(|c| IgdbCandidateDto {
            igdb_id: c.igdb_id,
            title: c.title,
            cover_url: c.cover_url,
            release_year: c.release_year,
        })
        .collect())
}

#[tauri::command]
pub async fn lookup_igdb_preview_by_id(
    auth: IgdbAuth,
    igdb_id: i64,
) -> Result<crate::services::igdb::IgdbEnrich, String> {
    use crate::services::igdb::{get_oauth_token, lookup_enrich_by_id};
    let token = get_oauth_token(&auth.client_id, &auth.client_secret)
        .await
        .map_err(|e| e.to_string())?;
    match lookup_enrich_by_id(&auth.client_id, &token, igdb_id).await {
        Ok(Some(info)) => Ok(info),
        Ok(None) => Err("No IGDB match".to_string()),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
pub async fn delete_game(db: tauri::State<'_, Db>, id: String) -> Result<(), String> {
    let game = fetch_game_audit_record(db.inner(), &id)
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query("DELETE FROM games WHERE id = ?")
        .bind(&id)
        .execute(db.inner())
        .await
        .map_err(|e| e.to_string())?;

    logging::log_db_transaction(format!(
        "Game deleted {}",
        format_game_audit_record(game.as_ref(), &id)
    ));
    Ok(())
}

#[tauri::command]
pub async fn hide_game(db: tauri::State<'_, Db>, id: String) -> Result<(), String> {
    let game = fetch_game_audit_record(db.inner(), &id)
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query(
        "UPDATE games
         SET hidden_at = COALESCE(hidden_at, CURRENT_TIMESTAMP),
             updated_at = CURRENT_TIMESTAMP
         WHERE id = ?",
    )
    .bind(&id)
    .execute(db.inner())
    .await
    .map_err(|e| e.to_string())?;

    logging::log_db_transaction(format!(
        "Game hidden {}",
        format_game_audit_record(game.as_ref(), &id)
    ));
    Ok(())
}

#[tauri::command]
pub async fn unhide_game(db: tauri::State<'_, Db>, id: String) -> Result<(), String> {
    let game = fetch_game_audit_record(db.inner(), &id)
        .await
        .map_err(|e| e.to_string())?;
    sqlx::query(
        "UPDATE games
         SET hidden_at = NULL,
             updated_at = CURRENT_TIMESTAMP
         WHERE id = ?",
    )
    .bind(&id)
    .execute(db.inner())
    .await
    .map_err(|e| e.to_string())?;

    logging::log_db_transaction(format!(
        "Game unhidden {}",
        format_game_audit_record(game.as_ref(), &id)
    ));
    Ok(())
}

// ================= Epic (Legendary) =================

#[derive(serde::Serialize)]
pub struct EpicOwnedGame {
    pub app_name: String,
    pub title: String,
}

fn clean_legendary_title(s: &str) -> String {
    s.trim_start()
        .trim_start_matches(|c: char| matches!(c, '*' | '+' | '•' | '·' | '●' | '►' | '▶'))
        .trim_start()
        .to_string()
}

#[tauri::command]
pub async fn fetch_epic_owned_via_legendary(
    legendary_path: Option<String>,
) -> Result<Vec<EpicOwnedGame>, String> {
    use std::process::Command;

    // Prefer a provided path; otherwise try common names in PATH.
    let candidates: Vec<String> = if let Some(lp) = legendary_path {
        vec![lp]
    } else {
        vec!["legendary".into(), "legendary.exe".into()]
    };

    let mut last_err = String::new();

    for bin in candidates {
        // 1) Try JSON mode first
        let out = Command::new(&bin).args(["list-games", "--json"]).output();
        match out {
            Ok(o) => {
                if o.status.success() {
                    let s = String::from_utf8_lossy(&o.stdout);
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&s) {
                        if let Some(arr) = val.as_array() {
                            let mut res = Vec::new();
                            for it in arr {
                                let title = clean_legendary_title(
                                    it.get("title").and_then(|v| v.as_str()).unwrap_or(""),
                                );
                                let app_name = it
                                    .get("app_name")
                                    .or_else(|| it.get("appname"))
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .trim()
                                    .to_string();
                                if !title.is_empty() && !app_name.is_empty() {
                                    res.push(EpicOwnedGame { title, app_name });
                                }
                            }
                            if !res.is_empty() {
                                return Ok(res);
                            }
                        }
                    }
                } else {
                    last_err = format!("{} exited with {}", bin, o.status);
                }
            }
            Err(e) => {
                last_err = e.to_string();
            }
        }

        // 2) Fallback: plaintext parse
        let out2 = Command::new(&bin).args(["list-games"]).output();
        match out2 {
            Ok(o) => {
                if o.status.success() {
                    let s = String::from_utf8_lossy(&o.stdout);
                    let mut res = Vec::new();
                    for line in s.lines() {
                        let line = line.trim();
                        if line.is_empty() {
                            continue;
                        }
                        // Skip headings or separators
                        let lower = line.to_lowercase();
                        if lower.contains("owned") && lower.contains("game") {
                            continue;
                        }
                        if line.starts_with('—') || line.starts_with("--") {
                            continue;
                        }

                        // Pattern 1: "Title (AppName)"
                        if let (Some(l), Some(r)) = (line.rfind('('), line.rfind(')')) {
                            if r > l {
                                let title = clean_legendary_title(line[..l].trim());
                                let app_name = line[l + 1..r].trim().to_string();
                                if !title.is_empty() && !app_name.is_empty() {
                                    res.push(EpicOwnedGame { title, app_name });
                                    continue;
                                }
                            }
                        }
                        // Pattern 2: "Title - AppName"
                        if let Some(idx) = line.rfind(" - ") {
                            let title = clean_legendary_title(line[..idx].trim());
                            let app_name = line[idx + 3..].trim().to_string();
                            if !title.is_empty() && !app_name.is_empty() {
                                res.push(EpicOwnedGame { title, app_name });
                                continue;
                            }
                        }
                    }
                    if !res.is_empty() {
                        return Ok(res);
                    }
                } else {
                    last_err = format!("{} exited with {}", bin, o.status);
                }
            }
            Err(e) => {
                last_err = e.to_string();
            }
        }
    }

    Err(format!(
        "Could not run Legendary. Is it installed and in PATH? Last error: {}",
        last_err
    ))
}

// ================ Xbox (OWNED games) via Microsoft Device Code ================

/// Begin device-code sign-in (show user_code + URL in the UI).
// --- Xbox device-code: begin ---
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct XboxDeviceCodeBegin {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    #[serde(default)]
    pub verification_uri_complete: Option<String>,
    pub expires_in: i64,
    #[serde(default)]
    pub interval: Option<i64>,
    #[serde(default)]
    pub message: Option<String>,
}

#[tauri::command]
pub async fn xbox_begin_device_code(client_id: String) -> Result<XboxDeviceCodeBegin, String> {
    let client_id = client_id.trim().to_string();
    if client_id.is_empty() {
        return Err("Microsoft Application (client) ID is required.".to_string());
    }

    let url = "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";
    let form = [
        ("client_id", client_id.as_str()),
        ("scope", "XboxLive.signin offline_access openid profile"),
    ];

    let client = reqwest::Client::new();
    let resp = client
        .post(url)
        .form(&form)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(format!(
            "Device-code begin failed: HTTP {} – {}",
            status,
            body.chars().take(300).collect::<String>()
        ));
    }

    let dc: XboxDeviceCodeBegin =
        serde_json::from_str(&body).map_err(|e| format!("Device-code JSON parse error: {}", e))?;
    Ok(dc)
}

fn build_client(
    default_headers: reqwest::header::HeaderMap,
    insecure: bool,
) -> Result<reqwest::Client, String> {
    // Build a client that (on Windows) trusts the OS cert store (incl. enterprise roots).
    let mut b = reqwest::Client::builder()
        .use_native_tls()
        .default_headers(default_headers)
        .redirect(reqwest::redirect::Policy::limited(5));

    if insecure {
        // DEV ONLY: lets you get past AV SSL inspection temporarily
        b = b.danger_accept_invalid_certs(true);
    }

    b.build().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn xbox_finish_device_code_and_import_owned(
    app: tauri::AppHandle,
    db: tauri::State<'_, crate::db::Db>,
    device_code: String,
    client_id: String,
    market: Option<String>,
    language: Option<String>,
) -> Result<u32, String> {
    use reqwest::header::{
        HeaderMap, HeaderValue, ACCEPT, ACCEPT_LANGUAGE, AUTHORIZATION, CONTENT_TYPE, USER_AGENT,
    };
    use std::collections::HashSet;
    use tokio::time::{sleep, Duration};

    // Optional debug escape hatch: set GL_XBOX_INSECURE_TLS=1 to bypass MITM during testing
    let insecure = std::env::var("GL_XBOX_INSECURE_TLS")
        .ok()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let market = market.unwrap_or_else(|| "US".to_string()).to_uppercase();
    let language = {
        let s = language.unwrap_or_else(|| "en-US".to_string());
        let mut parts = s.splitn(2, '-');
        let a = parts.next().unwrap_or("en").to_lowercase();
        let b = parts.next().unwrap_or("US").to_uppercase();
        format!("{}-{}", a, b)
    };
    // We'll send a weighted fallback too, as some services check for a list
    let accept_lang = format!("{},en;q=0.8", language);

    // ---------- 1) Poll AAD device code for access_token ----------
    let client_id = client_id.trim().to_string();
    if client_id.is_empty() {
        return Err("Microsoft Application (client) ID is required.".to_string());
    }
    let token_url = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";

    let mut h_basic = HeaderMap::new();
    h_basic.insert(USER_AGENT, HeaderValue::from_static("GameLexicon/1.0"));
    h_basic.insert(ACCEPT, HeaderValue::from_static("application/json"));
    // (Not required here, but harmless to include)
    h_basic.insert(
        ACCEPT_LANGUAGE,
        HeaderValue::from_str(&accept_lang).unwrap_or(HeaderValue::from_static("en-US,en;q=0.8")),
    );
    let client_basic = build_client(h_basic.clone(), insecure)?;

    let mut token: Option<serde_json::Value> = None;
    let mut wait = Duration::from_secs(5);
    for _ in 0..90 {
        let form = [
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ("client_id", client_id.as_str()),
            ("device_code", device_code.as_str()),
        ];
        let resp = client_basic
            .post(token_url)
            .form(&form)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();

        if !status.is_success() {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) {
                match v.get("error").and_then(|e| e.as_str()) {
                    Some("authorization_pending") => {
                        if let Some(iv) = v.get("interval").and_then(|x| x.as_i64()) {
                            wait = Duration::from_secs(iv.max(1) as u64);
                        }
                        sleep(wait).await;
                        continue;
                    }
                    Some("slow_down") => {
                        wait += Duration::from_secs(2);
                        sleep(wait).await;
                        continue;
                    }
                    Some(code) => return Err(format!("Device-code token error: {} – {}", code, v)),
                    None => {
                        return Err(format!(
                            "Device-code token failed: HTTP {} – {}",
                            status, body
                        ))
                    }
                }
            }
            return Err(format!(
                "Device-code token failed: HTTP {} – {}",
                status, body
            ));
        } else {
            token = serde_json::from_str::<serde_json::Value>(&body).ok();
            break;
        }
    }
    let token = token.ok_or_else(|| "Timed out waiting for sign-in approval.".to_string())?;
    let access_token = token
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "access_token missing in token response".to_string())?;

    // ---------- 2) XBL user.authenticate ----------
    let mut h_xbl = HeaderMap::new();
    h_xbl.insert(USER_AGENT, HeaderValue::from_static("GameLexicon/1.0"));
    h_xbl.insert(ACCEPT, HeaderValue::from_static("application/json"));
    h_xbl.insert("x-xbl-contract-version", HeaderValue::from_static("1"));
    h_xbl.insert(
        ACCEPT_LANGUAGE,
        HeaderValue::from_str(&accept_lang).unwrap_or(HeaderValue::from_static("en-US,en;q=0.8")),
    );
    let client_xbl = build_client(h_xbl.clone(), insecure)?;

    let user_auth_url = "https://user.auth.xboxlive.com/user/authenticate";
    let ua_body = serde_json::json!({
        "Properties": {
            "AuthMethod": "RPS",
            "SiteName": "user.auth.xboxlive.com",
            "RpsTicket": format!("d={}", access_token),
        },
        "RelyingParty": "http://auth.xboxlive.com",
        "TokenType": "JWT"
    });

    let resp = client_xbl
        .post(user_auth_url)
        .header(CONTENT_TYPE, "application/json")
        .json(&ua_body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(format!(
            "XBL user.authenticate failed: HTTP {} – {}",
            status,
            body.chars().take(300).collect::<String>()
        ));
    }
    let v: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("user.authenticate JSON parse: {}", e))?;
    let user_token = v
        .get("Token")
        .and_then(|x| x.as_str())
        .ok_or_else(|| "user.authenticate: Token missing".to_string())?;
    let uhs = v
        .get("DisplayClaims")
        .and_then(|d| d.get("xui"))
        .and_then(|x| x.get(0))
        .and_then(|x| x.get("uhs"))
        .and_then(|x| x.as_str())
        .ok_or_else(|| "user.authenticate: uhs missing".to_string())?;

    // ---------- 3) XSTS authorize ----------
    let xsts_url = "https://xsts.auth.xboxlive.com/xsts/authorize";
    let xsts_body = serde_json::json!({
        "Properties": { "SandboxId": "RETAIL", "UserTokens": [ user_token ] },
        "RelyingParty": "http://xboxlive.com",
        "TokenType": "JWT"
    });
    let resp = client_xbl
        .post(xsts_url)
        .header(CONTENT_TYPE, "application/json")
        .json(&xsts_body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        if let Ok(errv) = serde_json::from_str::<serde_json::Value>(&body) {
            let xerr = errv
                .get("XErr")
                .and_then(|x| x.as_i64())
                .unwrap_or_default();
            let msg = errv.get("Message").and_then(|x| x.as_str()).unwrap_or("");
            let friendly = match xerr {
                2148916233 => "Child/teen account needs family approval.",
                2148916235 => "Account’s country/region not supported.",
                2148916238 => "No Xbox Live profile yet—sign in at xbox.com once.",
                _ => "XSTS refused the token. Ensure a consumer Microsoft account.",
            };
            return Err(format!(
                "XSTS authorize failed (HTTP {}): XErr={} – {} (server: {})",
                status, xerr, friendly, msg
            ));
        }
        return Err(format!("XSTS authorize failed: HTTP {} {}", status, body));
    }
    let v: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("xsts JSON parse: {}", e))?;
    let xsts_token = v
        .get("Token")
        .and_then(|x| x.as_str())
        .ok_or_else(|| "xsts: Token missing".to_string())?;
    let xuid = v
        .get("DisplayClaims")
        .and_then(|d| d.get("xui"))
        .and_then(|x| x.get(0))
        .and_then(|x| x.get("xid"))
        .and_then(|x| x.as_str())
        .unwrap_or_default()
        .to_string();

    let auth_val = format!("XBL3.0 x={};{}", uhs, xsts_token);
    let auth_hdr = HeaderValue::from_str(&auth_val).map_err(|e| e.to_string())?;

    // ---------- 4) TitleHub (OWNED-first) ----------
    let mut h_th = HeaderMap::new();
    h_th.insert(USER_AGENT, HeaderValue::from_static("GameLexicon/1.0"));
    h_th.insert(ACCEPT, HeaderValue::from_static("application/json"));
    h_th.insert("x-xbl-contract-version", HeaderValue::from_static("2"));
    h_th.insert(AUTHORIZATION, auth_hdr.clone());
    h_th.insert(
        ACCEPT_LANGUAGE,
        HeaderValue::from_str(&accept_lang).unwrap_or(HeaderValue::from_static("en-US,en;q=0.8")),
    );
    let client_th = build_client(h_th, insecure)?;

    let th_urls = vec![
        // ✅ OWNED only
        format!("https://titlehub.xboxlive.com/users/xuid({})/titles?titleType=Owned&maxItems=2000", xuid),
        "https://titlehub.xboxlive.com/users/me/titles?titleType=Owned&maxItems=2000".to_string(),

        // Fallbacks
        format!("https://titlehub.xboxlive.com/users/xuid({})/titles?titleType=All&maxItems=2000", xuid),
        format!("https://titlehub.xboxlive.com/users/xuid({})/titles/titlehistory/decoration/boxart?maxItems=2000", xuid),
    ];

    #[derive(Clone, Debug)]
    struct ThItem {
        name: String,
        product_id: Option<String>,
        title_id: Option<String>,
        boxart: Option<String>,
    }

    let mut th_items: Vec<ThItem> = Vec::new();
    let mut last_status: Option<u16> = None;
    let mut last_body: String = String::new();

    for url in th_urls {
        println!("[xbox] TH GET {}", &url);
        match client_th.get(&url).send().await {
            Ok(resp) => {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                if !status.is_success() {
                    last_status = Some(status.as_u16());
                    last_body = body.chars().take(500).collect::<String>();
                    println!(
                        "[xbox] TH status={} body[0..300]={}",
                        status,
                        last_body.chars().take(300).collect::<String>()
                    );
                    continue;
                }
                match serde_json::from_str::<serde_json::Value>(&body) {
                    Ok(val) => {
                        let mut items_here = Vec::<ThItem>::new();
                        if let Some(arr) = val.get("titles").and_then(|x| x.as_array()) {
                            for t in arr {
                                let name = t
                                    .get("name")
                                    .and_then(|x| x.as_str())
                                    .unwrap_or_default()
                                    .trim()
                                    .to_string();
                                if name.is_empty() {
                                    continue;
                                }

                                let product_id = t
                                    .get("detail")
                                    .and_then(|d| d.get("productId"))
                                    .and_then(|x| x.as_str())
                                    .or_else(|| t.get("productId").and_then(|x| x.as_str()))
                                    .map(|s| s.trim().to_string());

                                let title_id = t
                                    .get("titleId")
                                    .map(|idv| {
                                        if let Some(s) = idv.as_str() {
                                            s.to_string()
                                        } else if let Some(n) = idv.as_u64() {
                                            n.to_string()
                                        } else {
                                            String::new()
                                        }
                                    })
                                    .filter(|s| !s.is_empty());

                                let boxart = t
                                    .get("displayImage")
                                    .and_then(|x| x.as_str())
                                    .map(|s| s.to_string())
                                    .or_else(|| {
                                        t.get("boxArt")
                                            .and_then(|b| b.get("url"))
                                            .and_then(|x| x.as_str())
                                            .map(|s| s.to_string())
                                    });

                                items_here.push(ThItem {
                                    name,
                                    product_id,
                                    title_id,
                                    boxart,
                                });
                            }
                        }

                        if !items_here.is_empty() {
                            println!("[xbox] TH got {} titles from {}", items_here.len(), &url);
                            th_items = items_here;
                            break;
                        } else {
                            last_status = Some(200);
                            last_body = "success but 0 titles parsed".to_string();
                            println!("[xbox] TH parsed 0 titles from success response");
                        }
                    }
                    Err(e) => {
                        last_status = Some(200);
                        last_body = format!("Parse error: {}", e);
                        println!("[xbox] TH JSON parse error: {}", e);
                    }
                }
            }
            Err(e) => {
                last_status = Some(0);
                last_body = format!("network error: {}", e);
                println!("[xbox] TH fetch error: {:?}", e);
            }
        }
    }

    if th_items.is_empty() {
        if let Some(st) = last_status {
            return Err(format!(
                "TitleHub owned titles failed (HTTP {}): {}",
                st,
                if last_body.is_empty() {
                    "no response body"
                } else {
                    &last_body
                }
            ));
        }
        return Err("TitleHub owned titles failed: no data. This can be caused by local HTTPS filtering (AV/SSL inspection) blocking *.xboxlive.com. Try another network or whitelist xboxlive domains.".to_string());
    }

    // ---------- 5) Hydrate via Display Catalog when productIds exist ----------
    #[derive(serde::Deserialize)]
    struct DcImage {
        #[serde(default)]
        image_type: String,
        #[serde(default)]
        uri: String,
    }
    #[derive(serde::Deserialize)]
    struct DcLocProp {
        #[serde(default)]
        product_title: String,
    }
    #[derive(serde::Deserialize)]
    struct DcMarketProp {
        #[serde(default)]
        original_release_date: Option<String>,
    }
    #[derive(serde::Deserialize)]
    struct DcProduct {
        #[serde(rename = "ProductId")]
        product_id: String,
        #[serde(rename = "Images", default)]
        images: Vec<DcImage>,
        #[serde(rename = "LocalizedProperties", default)]
        localized: Vec<DcLocProp>,
        #[serde(rename = "MarketProperties", default)]
        market_props: Vec<DcMarketProp>,
    }
    #[derive(serde::Deserialize)]
    struct DcResp {
        #[serde(rename = "Products", default)]
        products: Vec<DcProduct>,
    }

    let mut h_dc = HeaderMap::new();
    h_dc.insert(USER_AGENT, HeaderValue::from_static("GameLexicon/1.0"));
    h_dc.insert(ACCEPT, HeaderValue::from_static("application/json"));
    h_dc.insert("MS-CV", HeaderValue::from_static("DGU1mcuYo0WMMp+"));
    h_dc.insert(
        ACCEPT_LANGUAGE,
        HeaderValue::from_str(&accept_lang).unwrap_or(HeaderValue::from_static("en-US,en;q=0.8")),
    );
    let client_dc = build_client(h_dc, insecure)?;

    fn pick_cover(images: &[DcImage]) -> Option<String> {
        for want in ["Poster", "BrandedKeyArt", "SuperHeroArt", "Tile"] {
            if let Some(u) = images
                .iter()
                .find(|i| i.image_type.eq_ignore_ascii_case(want) && !i.uri.is_empty())
                .map(|i| i.uri.clone())
            {
                return Some(u);
            }
        }
        images
            .iter()
            .find(|i| !i.uri.is_empty())
            .map(|i| i.uri.clone())
    }
    fn parse_year(s: Option<&str>) -> Option<i64> {
        s.and_then(|v| v.get(0..4))
            .and_then(|y| y.parse::<i64>().ok())
            .filter(|y| *y > 1970)
    }

    let mut imported = 0_u32;
    let mut already_upserted: HashSet<String> = HashSet::new();

    let mut product_ids: Vec<String> = th_items
        .iter()
        .filter_map(|t| t.product_id.clone())
        .collect();
    product_ids.sort();
    product_ids.dedup();

    for chunk in product_ids.chunks(60) {
        let url = format!(
            "https://displaycatalog.mp.microsoft.com/v7.0/products?bigIds={}&market={}&languages={}",
            chunk.join(","), market, language
        );
        println!("[xbox] DC GET {}", url);
        let resp = client_dc
            .get(&url)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            println!(
                "[xbox] DC status={} body[0..300]={}",
                status,
                body.chars().take(300).collect::<String>()
            );
            continue;
        }
        let data: DcResp = resp.json().await.map_err(|e| e.to_string())?;

        for p in data.products {
            let title = p
                .localized
                .get(0)
                .map(|lp| lp.product_title.trim().to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| p.product_id.clone());

            let cover_url = pick_cover(&p.images);
            let release_year = parse_year(
                p.market_props
                    .get(0)
                    .and_then(|m| m.original_release_date.as_deref()),
            );

            sqlx::query(r#"
                INSERT INTO games (id, canonical_title, platform, storefront_id, owned_source,
                                   license_type, license_source, cover_url, release_year, updated_at)
                VALUES (?, ?, 'Xbox', ?, 'Xbox Account',
                        'owned', NULL, ?, ?, CURRENT_TIMESTAMP)
                ON CONFLICT(platform, storefront_id) DO UPDATE SET
                    canonical_title = excluded.canonical_title,
                    cover_url = COALESCE(excluded.cover_url, games.cover_url),
                    release_year = COALESCE(excluded.release_year, games.release_year),
                    license_type = excluded.license_type,
                    license_source = excluded.license_source,
                    updated_at = CURRENT_TIMESTAMP
            "#)
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(&title)
            .bind(&p.product_id)
            .bind(cover_url)
            .bind(release_year)
            .execute(db.inner())
            .await
            .map_err(|e| e.to_string())?;

            already_upserted.insert(p.product_id.clone());
            imported += 1;
        }
        sleep(Duration::from_millis(200)).await;
    }

    // Insert remaining TitleHub-only rows (no productId/DC miss)
    for it in th_items {
        let storefront_id = if let Some(pid) = it.product_id.clone() {
            if already_upserted.contains(&pid) {
                continue;
            }
            pid
        } else if let Some(tid) = it.title_id.clone() {
            format!("TH-{}", tid)
        } else {
            continue;
        };

        sqlx::query(
            r#"
            INSERT INTO games (id, canonical_title, platform, storefront_id, owned_source,
                               license_type, license_source, cover_url, release_year, updated_at)
            VALUES (?, ?, 'Xbox', ?, 'Xbox Account',
                    'owned', NULL, ?, NULL, CURRENT_TIMESTAMP)
            ON CONFLICT(platform, storefront_id) DO UPDATE SET
                canonical_title = excluded.canonical_title,
                cover_url = COALESCE(excluded.cover_url, games.cover_url),
                license_type = excluded.license_type,
                license_source = excluded.license_source,
                updated_at = CURRENT_TIMESTAMP
        "#,
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&it.name)
        .bind(&storefront_id)
        .bind(it.boxart.clone())
        .execute(db.inner())
        .await
        .map_err(|e| e.to_string())?;

        imported += 1;
    }

    println!(
        "[import] Xbox OWNED (TitleHub-first) imported/updated {} rows (market={}, lang={}).",
        imported, market, language
    );
    logging::log_db_transaction(format!(
        "Xbox owned import completed imported_or_updated={} market={} language={}",
        imported,
        quote_for_log(&market, 12),
        quote_for_log(&language, 16)
    ));
    let _ = app.emit("xbox_owned_import_done", imported);
    Ok(imported)
}
