//! User settings, a small JSON file in the app config directory
//! (`~/.config/com.questbacklog.QuestBacklog/settings.json` on Linux,
//! `%APPDATA%\com.questbacklog.QuestBacklog\settings.json` on Windows), edited
//! from the overlay's Preferences screen.

use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Shows or hides the overlay, e.g. "Ctrl+Shift+G". On Wayland it is
    /// only the preferred trigger; the desktop's shortcut settings decide.
    pub hotkey: String,
    /// Takes a screenshot and opens the post composer with it attached.
    pub screenshot_hotkey: String,
    /// Keep a PNG copy of every screenshot on this computer.
    pub save_screenshots: bool,
    /// Where copies go; `None` means `<Pictures>/Quest Backlog`.
    pub screenshot_folder: Option<String>,
    /// "en" or "es"; anything else follows the system language.
    pub language: Option<String>,
    /// The quick panel's first-run shortcut tips were dismissed.
    pub tips_seen: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            hotkey: "Ctrl+Shift+G".to_string(),
            // Not Ctrl+Shift+S: a global shortcut would steal "Save as"
            // from every other app.
            screenshot_hotkey: "Ctrl+Shift+F12".to_string(),
            save_screenshots: false,
            screenshot_folder: None,
            language: None,
            tips_seen: false,
        }
    }
}

pub struct SettingsState(pub Mutex<Settings>);

pub fn current(app: &AppHandle) -> Settings {
    app.state::<SettingsState>().0.lock().unwrap().clone()
}

fn path(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|dir| dir.join("settings.json"))
}

pub fn load(app: &AppHandle) -> Settings {
    let Some(path) = path(app) else {
        return Settings::default();
    };
    match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_else(|error| {
            eprintln!("[settings] {} is invalid, using defaults: {error}", path.display());
            Settings::default()
        }),
        Err(_) => {
            let settings = Settings::default();
            if let Err(error) = write(app, &settings) {
                eprintln!("[settings] could not write {}: {error}", path.display());
            }
            settings
        }
    }
}

fn write(app: &AppHandle, settings: &Settings) -> Result<(), String> {
    let path = path(app).ok_or("no config directory")?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    }
    let text = serde_json::to_string_pretty(settings).map_err(|error| error.to_string())?;
    std::fs::write(&path, text).map_err(|error| error.to_string())
}

/// Saves to disk and makes `settings` current.
pub fn store(app: &AppHandle, settings: Settings) -> Result<(), String> {
    write(app, &settings)?;
    *app.state::<SettingsState>().0.lock().unwrap() = settings;
    Ok(())
}

/// "es" or "en".
pub fn language(settings: &Settings) -> &'static str {
    let chosen = settings
        .language
        .clone()
        .filter(|language| language == "en" || language == "es")
        .or_else(sys_locale::get_locale)
        .unwrap_or_default()
        .to_lowercase();
    if chosen.starts_with("es") {
        "es"
    } else {
        "en"
    }
}

/// The folder screenshot copies go to, chosen or default.
pub fn screenshot_folder(app: &AppHandle, settings: &Settings) -> Option<PathBuf> {
    settings
        .screenshot_folder
        .as_deref()
        .map(str::trim)
        .filter(|folder| !folder.is_empty())
        .map(PathBuf::from)
        .or_else(|| app.path().picture_dir().ok().map(|dir| dir.join("Quest Backlog")))
}
