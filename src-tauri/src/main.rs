// Quest Backlog desktop app. See README.md. All HTTP, the device token,
// hotkeys, screenshots and URL opening live in Rust; the webview renders
// state and calls the commands below.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod api;
mod auth;
mod compose;
mod config;
mod detect;
mod device;
mod feed;
mod games;
mod hotkey;
mod known_games;
mod overlay;
mod preferences;
mod screenshot;
mod secret;
mod settings;
mod tray;

use serde::Serialize;
use tauri::{AppHandle, Manager, RunEvent, State};
use tauri_plugin_opener::OpenerExt;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppInfo {
    version: &'static str,
    language: &'static str,
    hotkey: hotkey::HotkeyStatus,
    /// Whether the first-run shortcut tips were dismissed.
    tips_seen: bool,
}

#[tauri::command]
fn get_app_info(app: AppHandle) -> AppInfo {
    let settings = settings::current(&app);
    AppInfo {
        version: config::APP_VERSION,
        language: settings::language(&settings),
        hotkey: hotkey::status(&app),
        tips_seen: settings.tips_seen,
    }
}

#[tauri::command]
fn dismiss_tips(app: AppHandle) -> Result<(), String> {
    let mut settings = settings::current(&app);
    if settings.tips_seen {
        return Ok(());
    }
    settings.tips_seen = true;
    settings::store(&app, settings)
}

#[tauri::command]
fn get_auth(auth: State<auth::Auth>) -> auth::AuthView {
    auth.view()
}

#[tauri::command]
async fn start_sign_in(app: AppHandle) {
    auth::start_sign_in(&app).await;
}

#[tauri::command]
fn cancel_sign_in(app: AppHandle) {
    auth::cancel_sign_in(&app);
}

#[tauri::command]
async fn retry_auth(app: AppHandle) {
    auth::retry(&app).await;
}

#[tauri::command]
async fn sign_out(app: AppHandle) {
    auth::sign_out(&app).await;
}

/// Opens the pending /link page in the browser. The URL is built in Rust
/// from the configured web origin, never taken from the webview. The
/// overlay hides so the browser can take input.
#[tauri::command]
fn open_sign_in_page(app: AppHandle, auth: State<auth::Auth>) -> Result<(), String> {
    let url = auth.link_url().ok_or("no sign-in in progress")?;
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|error| error.to_string())?;
    overlay::hide(&app);
    Ok(())
}

#[tauri::command]
fn hide_overlay(app: AppHandle) {
    overlay::hide(&app);
}

/// Passed when the system starts the app at login (Preferences option).
const AUTOSTART_FLAG: &str = "--autostart";

fn wants_toggle(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "--toggle")
}

fn main() {
    tauri::Builder::default()
        // Must be the first plugin. A second launch toggles (--toggle),
        // takes a screenshot of the game (--screenshot) or shows the overlay
        // of the running app instead of starting another.
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if wants_toggle(&args) {
                overlay::toggle(app);
            } else if args.iter().any(|arg| arg == "--screenshot") {
                compose::screenshot(app, screenshot::Target::ActiveWindow);
            } else {
                overlay::show(app);
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![AUTOSTART_FLAG]),
        ))
        .manage(auth::Auth::new(api::Api::new(config::api_base())))
        .manage(hotkey::HotkeyState::default())
        .manage(games::GamesState::default())
        .manage(compose::ComposeState::default())
        .manage(feed::FeedState::default())
        .invoke_handler(tauri::generate_handler![
            compose::get_screenshot,
            compose::take_screenshot,
            compose::discard_screenshot,
            compose::crop_screenshot,
            compose::create_post,
            compose::open_post,
            preferences::open_preferences,
            preferences::close_preferences,
            preferences::get_preferences,
            preferences::update_preferences,
            preferences::choose_screenshot_folder,
            preferences::configure_shortcuts,
            games::detect_game,
            games::search_games,
            games::choose_game,
            games::set_game_status,
            games::share_status,
            feed::feed_page,
            feed::my_posts_page,
            feed::like_post,
            feed::post_comments,
            feed::add_comment,
            get_app_info,
            dismiss_tips,
            get_auth,
            start_sign_in,
            cancel_sign_in,
            retry_auth,
            sign_out,
            open_sign_in_page,
            hide_overlay,
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            let settings = settings::load(&handle);
            let language = settings::language(&settings);
            app.manage(settings::SettingsState(std::sync::Mutex::new(settings.clone())));

            overlay::setup(&handle);
            tray::setup(&handle, language)?;
            hotkey::setup(&handle, &settings);

            // Started by the user (launcher, terminal): open the panel, as a
            // second launch does. Started at login: stay in the tray.
            let autostarted = std::env::args().any(|arg| arg == AUTOSTART_FLAG);
            tauri::async_runtime::spawn(async move {
                auth::restore(&handle).await;
                if !autostarted {
                    overlay::show(&handle);
                }
            });
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building the Quest Backlog desktop app")
        .run(|_app, event| {
            // The overlay hides instead of closing; only the tray's Quit exits.
            if let RunEvent::ExitRequested { api, code: None, .. } = event {
                api.prevent_exit();
            }
        });
}
