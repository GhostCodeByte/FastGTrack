use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use chrono::Local;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub accent_color: String,
    pub background_color: String,
    pub text_color: String,
    pub default_rest_seconds: i32,
    pub default_weight_unit: String,
    pub default_set_count: i32,
    pub default_rep_target: i32,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            accent_color: "#f2cf00".into(),
            background_color: "#efede8".into(),
            text_color: "#111111".into(),
            default_rest_seconds: 90,
            default_weight_unit: "Kg".into(),
            default_set_count: 3,
            default_rep_target: 8,
        }
    }
}

pub fn app_storage_dir() -> PathBuf {
    dirs::data_local_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("FastGTrack")
}

pub fn settings_path() -> PathBuf {
    app_storage_dir().join("settings.json")
}

pub fn exports_dir() -> PathBuf {
    app_storage_dir().join("exports")
}

pub fn backups_dir() -> PathBuf {
    app_storage_dir().join("backups")
}

pub fn ensure_storage_dirs() -> Result<()> {
    fs::create_dir_all(app_storage_dir()).context("failed to create app storage dir")?;
    fs::create_dir_all(exports_dir()).context("failed to create export dir")?;
    fs::create_dir_all(backups_dir()).context("failed to create backup dir")?;
    Ok(())
}

pub fn load_settings() -> Result<AppSettings> {
    ensure_storage_dirs()?;
    let path = settings_path();
    if !path.exists() {
        let defaults = AppSettings::default();
        save_settings(&defaults)?;
        return Ok(defaults);
    }

    let content = fs::read_to_string(&path).context("failed to read settings file")?;
    let settings: AppSettings =
        serde_json::from_str(&content).context("failed to parse settings file")?;
    Ok(settings)
}

pub fn save_settings(settings: &AppSettings) -> Result<()> {
    ensure_storage_dirs()?;
    let content = serde_json::to_string_pretty(settings).context("failed to serialize settings")?;
    fs::write(settings_path(), content).context("failed to write settings file")?;
    Ok(())
}

pub fn normalize_hex_color(value: &str, fallback: &str) -> String {
    let trimmed = value.trim();
    let hex = trimmed.strip_prefix('#').unwrap_or(trimmed);
    if hex.len() == 6 && hex.chars().all(|ch| ch.is_ascii_hexdigit()) {
        format!("#{}", hex.to_ascii_uppercase())
    } else {
        fallback.to_string()
    }
}

pub fn timestamped_file(dir: PathBuf, prefix: &str) -> PathBuf {
    let stamp = Local::now().format("%Y%m%d-%H%M%S").to_string();
    dir.join(format!("{prefix}-{stamp}.json"))
}

pub fn latest_json_file(dir: PathBuf) -> Option<PathBuf> {
    let mut files = fs::read_dir(dir)
        .ok()?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json"))
        .collect::<Vec<_>>();
    files.sort();
    files.pop()
}
