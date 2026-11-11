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
    /// What the chat talks to: "api" (the connected service) or "claude-code".
    pub chat_backend: String,
    /// The address of a local AI server (Ollama, LM Studio …), when one is connected.
    pub local_url: String,
    /// The language of the menus and buttons (not of the CV): "auto" follows the computer, "no",
    /// "en", or the code of an installed language pack (see `languages_dir`).
    pub ui_language: String,
    /// "system", "light" or "dark".
    pub theme: String,
    /// Use the person's own checklist instead of the built-in recruiter check.
    pub own_checklist: bool,
    /// Save the CV to its file every five minutes while it has changes, like Word.
    pub autosave: bool,
    /// The file the CV was last opened from or saved to (empty: none), so «Lagre» and the
    /// autosave keep using it after a restart.
    pub cv_file: String,
    /// The first start's language question has been answered.
    pub welcomed: bool,
    /// The language a new CV starts in (its own name); empty: the computer's.
    pub cv_language: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            remember: true,
            ai_service: None,
            ai_model: String::new(),
            chat_backend: "api".into(),
            local_url: String::new(),
            ui_language: "auto".into(),
            theme: "system".into(),
            own_checklist: false,
            autosave: true,
            cv_file: String::new(),
            welcomed: false,
            cv_language: String::new(),
        }
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

// ---------- interface language packs: `<code>.cvlang.json` in the `languages` folder ----------

pub fn languages_dir() -> PathBuf {
    data_dir().join("languages")
}

/// The file name of a language pack.
pub fn language_file(code: &str) -> String {
    format!("{code}.cvlang.json")
}

/// The text of every installed language pack file.
pub fn language_files() -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(languages_dir()) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.to_string_lossy().ends_with(".cvlang.json")).collect();
    paths.sort();
    paths.iter().filter_map(|p| std::fs::read_to_string(p).ok()).collect()
}

/// Saves a language pack (write then rename, so a crash never leaves half a file).
pub fn save_language(code: &str, text: &str) -> Result<PathBuf, String> {
    let dir = languages_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let name = language_file(code);
    let tmp = dir.join(format!("{name}.tmp"));
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    let path = dir.join(name);
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
    Ok(path)
}

pub fn delete_language(code: &str) -> Result<(), String> {
    std::fs::remove_file(languages_dir().join(language_file(code))).map_err(|e| e.to_string())
}

/// The person has their own file for this language (added, imported or edited).
pub fn has_language(code: &str) -> bool {
    languages_dir().join(language_file(code)).is_file()
}
