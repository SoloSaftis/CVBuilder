//! Where the app keeps the remembered draft, a backup and settings.
//!
//! Portable mode: when a folder named `CVBuilder-data` sits next to the program, everything is
//! stored there (for a USB stick). Otherwise the user's own app-data folder is used.
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub fn data_dir() -> PathBuf {
    if let Some(dir) = std::env::current_exe().ok().and_then(|exe| exe.parent().map(|p| p.join("CVBuilder-data"))) {
        if dir.is_dir() {
            return dir;
        }
    }
    dirs::data_dir().unwrap_or_else(std::env::temp_dir).join("CVBuilder")
}

fn path(name: &str) -> PathBuf {
    data_dir().join(name)
}

pub fn portable() -> bool {
    std::env::current_exe().ok().and_then(|exe| exe.parent().map(|p| p.join("CVBuilder-data").is_dir())).unwrap_or(false)
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Settings {
    pub remember: bool,
    /// The connected AI service; the key itself lives in the OS keychain.
    pub ai_service: Option<cv_ai::Service>,
    pub ai_model: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { remember: true, ai_service: None, ai_model: String::new() }
    }
}

pub fn load_settings() -> Settings {
    std::fs::read_to_string(path("settings.json")).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

pub fn save_settings(settings: &Settings) {
    write("settings.json", &serde_json::to_string_pretty(settings).unwrap_or_default());
}

fn write(name: &str, text: &str) {
    let dir = data_dir();
    if std::fs::create_dir_all(&dir).is_ok() {
        // Write then rename, so a crash never leaves half a file.
        let tmp = dir.join(format!("{name}.tmp"));
        if std::fs::write(&tmp, text).is_ok() {
            let _ = std::fs::rename(&tmp, dir.join(name));
        }
    }
}

pub fn load_draft() -> Option<String> {
    std::fs::read_to_string(path("utkast.json")).ok()
}

pub fn save_draft(json: &str) {
    write("utkast.json", json);
}

pub fn forget_draft() {
    let _ = std::fs::remove_file(path("utkast.json"));
}

/// The CV that was replaced by «Ny tom CV» or «Vis eksempel», kept in case undo is not enough.
pub fn save_backup(json: &str) {
    write("sikkerhetskopi.json", json);
}
