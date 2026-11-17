//! The Preferences window: shortcuts, screenshot copies, language and
//! starting with the system. A normal window, opened from the tray or the
//! quick panel. It is created on open and destroyed on close, so its web
//! view (~190 MB) is not kept in memory while unused. Settings go through
//! `settings` and apply without a restart.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_dialog::DialogExt;

use crate::hotkey::{self, HotkeyStatus};
use crate::settings::{self, Settings};
use crate::{overlay, tray};

pub const LABEL: &str = "preferences";
/// Tells both windows to reload settings (language, shortcuts).
pub const CHANGED_EVENT: &str = "settings://changed";

fn window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(LABEL)
}

pub fn open(app: &AppHandle) {
    overlay::hide(app);
    if let Some(window) = window(app) {
        let _ = window.unminimize();
        let _ = window.set_focus();
        return;
    }
    let created = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html".into()))
        .title("Quest Backlog Preferences")
        .inner_size(520.0, 600.0)
        .min_inner_size(440.0, 480.0)
        .center()
        .focused(true)
        .build();
    if let Err(error) = created {
        eprintln!("[preferences] could not open the window: {error}");
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preferences {
    pub hotkey: String,
    pub screenshot_hotkey: String,
    pub save_screenshots: bool,
    /// The folder in use, chosen or default.
    pub screenshot_folder: Option<String>,
    pub language: Option<String>,
    /// Read from the system, which is the source of truth for it.
    pub start_with_system: bool,
    /// What is actually bound right now.
    pub hotkeys: HotkeyStatus,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreferencesInput {
    pub hotkey: String,
    pub screenshot_hotkey: String,
    pub save_screenshots: bool,
    pub screenshot_folder: Option<String>,
    pub language: Option<String>,
    pub start_with_system: bool,
}

fn starts_with_system(app: &AppHandle) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

fn view(app: &AppHandle, settings: &Settings) -> Preferences {
    Preferences {
        hotkey: settings.hotkey.clone(),
        screenshot_hotkey: settings.screenshot_hotkey.clone(),
        save_screenshots: settings.save_screenshots,
        screenshot_folder: settings::screenshot_folder(app, settings).map(|path| path.display().to_string()),
        language: settings.language.clone(),
        start_with_system: starts_with_system(app),
        hotkeys: hotkey::status(app),
    }
}

#[tauri::command]
pub fn open_preferences(app: AppHandle) {
    open(&app);
}

#[tauri::command]
pub fn close_preferences(app: AppHandle) {
    if let Some(window) = window(&app) {
        let _ = window.close();
    }
}

#[tauri::command]
pub fn get_preferences(app: AppHandle) -> Preferences {
    view(&app, &settings::current(&app))
}

/// Errors are UI message keys.
#[tauri::command]
pub fn update_preferences(app: AppHandle, input: PreferencesInput) -> Result<Preferences, String> {
    let previous = settings::current(&app);
    let system_managed = hotkey::status(&app).system_managed;

    let folder = input
        .screenshot_folder
        .map(|folder| folder.trim().to_string())
        .filter(|folder| !folder.is_empty());
    if folder.as_deref().is_some_and(|folder| !std::path::Path::new(folder).is_absolute()) {
        return Err("folderInvalid".to_string());
    }

    let next = Settings {
        // On Wayland the desktop owns the bindings; keep what we have.
        hotkey: if system_managed { previous.hotkey.clone() } else { input.hotkey.trim().to_string() },
        screenshot_hotkey: if system_managed {
            previous.screenshot_hotkey.clone()
        } else {
            input.screenshot_hotkey.trim().to_string()
        },
        save_screenshots: input.save_screenshots,
        screenshot_folder: folder,
        language: input.language.filter(|language| language == "en" || language == "es"),
        tips_seen: previous.tips_seen,
    };

    let hotkeys_changed =
        next.hotkey != previous.hotkey || next.screenshot_hotkey != previous.screenshot_hotkey;
    if hotkeys_changed {
        hotkey::apply(&app, &next)?;
    }
    if let Err(error) = settings::store(&app, next.clone()) {
        eprintln!("[settings] could not save: {error}");
        if hotkeys_changed {
            let _ = hotkey::apply(&app, &previous);
        }
        return Err("failed".to_string());
    }

    if input.start_with_system != starts_with_system(&app) {
        let autolaunch = app.autolaunch();
        let changed = if input.start_with_system { autolaunch.enable() } else { autolaunch.disable() };
        if let Err(error) = changed {
            eprintln!("[settings] could not change start with system: {error}");
            return Err("autostartFailed".to_string());
        }
    }

    tray::set_language(&app, settings::language(&next));
    let _ = app.emit(CHANGED_EVENT, ());
    Ok(view(&app, &next))
}

/// Folder picker for screenshot copies.
#[tauri::command]
pub async fn choose_screenshot_folder(app: AppHandle) -> Option<String> {
    let start = settings::screenshot_folder(&app, &settings::current(&app));
    let mut dialog = app.dialog().file();
    if let Some(parent) = window(&app) {
        dialog = dialog.set_parent(&parent);
    }
    if let Some(start) = start.filter(|path| path.is_dir()) {
        dialog = dialog.set_directory(start);
    }
    tauri::async_runtime::spawn_blocking(move || dialog.blocking_pick_folder())
        .await
        .ok()
        .flatten()
        .and_then(|path| path.into_path().ok())
        .map(|path| path.display().to_string())
}

/// Wayland: opens the desktop's shortcut dialog.
#[tauri::command]
pub async fn configure_shortcuts(app: AppHandle) -> Result<(), String> {
    hotkey::configure(&app).await
}
