//! Posting from the overlay: the screenshot flow and post creation.
//!
//! A screenshot of the game is taken from the shortcut or "Add screenshot";
//! "Choose window…" and the tray let the user click a window instead. It is
//! kept here as JPEG bytes, and only a preview reaches the webview. If the user turned it on in Preferences, a
//! full-quality PNG copy is saved on this computer.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_opener::OpenerExt;

use crate::api::{NewPost, PostError};
use crate::auth::{self, Auth};
use crate::screenshot::{CaptureError, Target};
use crate::{config, overlay, screenshot, settings};

pub const SCREENSHOT_EVENT: &str = "screenshot://changed";
/// Tells the UI to switch to the post composer.
pub const COMPOSE_EVENT: &str = "overlay://compose";

/// How long to wait after hiding the overlay before capturing.
const OVERLAY_SETTLE: Duration = Duration::from_millis(120);
/// How long KWin needs to move focus to the game when the overlay lets go
/// of the keyboard.
#[cfg(target_os = "linux")]
const FOCUS_SETTLE: Duration = Duration::from_millis(60);

/// The post length limit of the Quest Backlog API.
const POST_MAX_CHARS: usize = 500;

#[derive(Debug, Clone, Default, Serialize)]
#[serde(tag = "state", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ScreenshotView {
    #[default]
    None,
    Capturing,
    Ready {
        preview: String,
        /// Where the local copy went, when saving copies is on.
        saved_to: Option<String>,
        save_failed: bool,
    },
    Failed,
}

#[derive(Default)]
pub struct ComposeState {
    capturing: AtomicBool,
    jpeg: Mutex<Option<Vec<u8>>>,
    view: Mutex<ScreenshotView>,
}

fn set_view(app: &AppHandle, view: ScreenshotView) {
    *app.state::<ComposeState>().view.lock().unwrap() = view.clone();
    let _ = app.emit(SCREENSHOT_EVENT, view);
}

fn clear(app: &AppHandle) {
    *app.state::<ComposeState>().jpeg.lock().unwrap() = None;
    set_view(app, ScreenshotView::None);
}

/// Takes a screenshot of the game (or a window the user chooses), then
/// opens the composer with it. Ignored while a capture is already running.
pub fn screenshot(app: &AppHandle, target: Target) {
    let state = app.state::<ComposeState>();
    if state.capturing.swap(true, Ordering::SeqCst) {
        return;
    }
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        capture_flow(&handle, target).await;
        handle.state::<ComposeState>().capturing.store(false, Ordering::SeqCst);
    });
}

/// KDE: captures the game while the overlay stays up, by handing it
/// keyboard focus for a moment. `None` when that is not possible; the
/// caller then hides the overlay instead.
#[cfg(target_os = "linux")]
async fn capture_beside_overlay(app: &AppHandle) -> Option<image::DynamicImage> {
    if !overlay::is_wayland() {
        return None;
    }
    overlay::set_keyboard_exclusive(app, false);
    tokio::time::sleep(FOCUS_SETTLE).await;
    let image = screenshot::capture_active_window_kwin().await;
    overlay::set_keyboard_exclusive(app, true);
    // Still the overlay itself: KWin kept it active.
    image.filter(|image| Some((image.width(), image.height())) != overlay::physical_size(app))
}

#[cfg(not(target_os = "linux"))]
async fn capture_beside_overlay(_app: &AppHandle) -> Option<image::DynamicImage> {
    None
}

async fn capture_flow(app: &AppHandle, target: Target) {
    let was_visible = overlay::is_visible(app);
    let started = Instant::now();

    let mut kept_overlay = false;
    let captured = match (target, was_visible) {
        (Target::ActiveWindow, true) => match capture_beside_overlay(app).await {
            Some(image) => {
                kept_overlay = true;
                Ok(image)
            }
            None => capture_with_overlay_hidden(app, target).await,
        },
        (_, true) => capture_with_overlay_hidden(app, target).await,
        (_, false) => screenshot::capture(target).await,
    };
    let captured_in = started.elapsed();

    let image = match captured {
        Ok(image) => image,
        Err(CaptureError::Cancelled) => {
            // Back to where the user was; the previous screenshot stays.
            if was_visible {
                overlay::show(app);
            }
            return;
        }
        Err(CaptureError::Failed(error)) => {
            eprintln!("[screenshot] capture failed: {error}");
            set_view(app, ScreenshotView::Failed);
            open_composer(app);
            return;
        }
    };

    // The picture is taken: bring the composer up now and encode while it
    // shows "Preparing screenshot…".
    set_view(app, ScreenshotView::Capturing);
    if kept_overlay {
        let _ = app.emit(COMPOSE_EVENT, ());
    } else {
        open_composer(app);
    }

    let current = settings::current(app);
    let folder = if current.save_screenshots {
        settings::screenshot_folder(app, &current)
    } else {
        None
    };
    // The optional PNG copy is written alongside the upload encoding.
    let result = tauri::async_runtime::spawn_blocking(move || {
        std::thread::scope(|scope| {
            let saving = folder.as_deref().map(|folder| scope.spawn(|| screenshot::save_copy(&image, folder)));
            let prepared = screenshot::prepare(&image)?;
            let saved = saving.map(|saving| {
                saving.join().unwrap_or_else(|_| Err("saving the copy panicked".to_string()))
            });
            Ok::<_, String>((prepared, saved))
        })
    })
    .await
    .unwrap_or_else(|error| Err(error.to_string()));

    match result {
        Ok((prepared, saved)) => {
            let (saved_to, save_failed) = match saved {
                Some(Ok(path)) => (Some(path.display().to_string()), false),
                Some(Err(error)) => {
                    eprintln!("[screenshot] could not save a local copy: {error}");
                    (None, true)
                }
                None => (None, false),
            };
            eprintln!(
                "[screenshot] {target:?}{} captured in {} ms, ready in {} ms",
                if kept_overlay { " (overlay kept)" } else { "" },
                captured_in.as_millis(),
                started.elapsed().as_millis()
            );
            *app.state::<ComposeState>().jpeg.lock().unwrap() = Some(prepared.upload_jpeg);
            set_view(
                app,
                ScreenshotView::Ready { preview: prepared.preview_data_url, saved_to, save_failed },
            );
        }
        Err(error) => {
            eprintln!("[screenshot] could not prepare the screenshot: {error}");
            set_view(app, ScreenshotView::Failed);
        }
    }
}

async fn capture_with_overlay_hidden(
    app: &AppHandle,
    target: Target,
) -> Result<image::DynamicImage, CaptureError> {
    overlay::hide(app);
    // Let the compositor unmap the overlay and give focus back to the game.
    // KWin does it in under 50 ms.
    tokio::time::sleep(OVERLAY_SETTLE).await;
    screenshot::capture(target).await
}

fn open_composer(app: &AppHandle) {
    overlay::show(app);
    let _ = app.emit(COMPOSE_EVENT, ());
}

#[tauri::command]
pub fn get_screenshot(state: tauri::State<ComposeState>) -> ScreenshotView {
    state.view.lock().unwrap().clone()
}

#[tauri::command]
pub fn take_screenshot(app: AppHandle, choose_window: bool) {
    let target = if choose_window { Target::ChosenWindow } else { Target::ActiveWindow };
    screenshot(&app, target);
}

#[tauri::command]
pub fn discard_screenshot(app: AppHandle) {
    clear(&app);
}

/// Posts to the feed. Returns the new post's id. Errors are UI message
/// keys, or `server:<message>` when the API explains the refusal.
#[tauri::command]
pub async fn create_post(
    app: AppHandle,
    content: String,
    game_id: Option<String>,
    spoiler: bool,
    with_screenshot: bool,
) -> Result<String, String> {
    let token = auth::required_token(&app)?;
    let content = content.trim();
    if content.chars().count() > POST_MAX_CHARS {
        return Err("postTooLong".to_string());
    }
    let image = if with_screenshot {
        let jpeg = app.state::<ComposeState>().jpeg.lock().unwrap().clone();
        Some(jpeg.ok_or("screenshotMissing")?)
    } else {
        None
    };
    if content.is_empty() && image.is_none() {
        return Err("postEmpty".to_string());
    }
    let game_id = game_id.as_deref().map(str::trim).filter(|id| !id.is_empty());

    let post = NewPost { content, game_id, spoiler, image };
    match app.state::<Auth>().api().create_post(&token, post).await {
        Ok(created) => {
            if with_screenshot {
                clear(&app);
            }
            Ok(created.id)
        }
        Err(PostError::Rejected(message)) => Err(format!("server:{message}")),
        Err(PostError::Api(error)) => Err(auth::api_error(&app, error).await),
    }
}

/// Opens a post on the website and gets the overlay out of the way.
#[tauri::command]
pub fn open_post(app: AppHandle, post_id: String) -> Result<(), String> {
    let valid = !post_id.is_empty()
        && post_id.len() <= 64
        && post_id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if !valid {
        return Err("failed".to_string());
    }
    let url = format!("{}/post/{post_id}", config::web_base());
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|_| "failed".to_string())?;
    overlay::hide(&app);
    Ok(())
}
