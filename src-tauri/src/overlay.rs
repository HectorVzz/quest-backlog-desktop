//! The overlay window: a separate always-on-top window, never injected into
//! the game. On Wayland it is a wlr-layer-shell surface on the overlay layer
//! (verified on KDE Plasma), which is what keeps it
//! above borderless-fullscreen games. Exclusive fullscreen is not supported.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager, WebviewWindow, WindowEvent};

pub const LABEL: &str = "overlay";
pub const SHOWN_EVENT: &str = "overlay://shown";
/// The panel plays its closing animation; the window hides after `CLOSE_ANIMATION`.
pub const HIDING_EVENT: &str = "overlay://hiding";
/// Matches the closing animation in src/app.css.
const CLOSE_ANIMATION: Duration = Duration::from_millis(140);

/// Bumped on every show, so a delayed hide started before a show does not
/// hide the panel the user just opened again.
static SHOWN: AtomicU64 = AtomicU64::new(0);

fn window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(LABEL)
}

pub fn show(app: &AppHandle) {
    if let Some(window) = window(app) {
        // Windows: note the game in front before the overlay takes focus.
        crate::detect::capture_foreground();
        eprintln!("[overlay] show (visible before: {})", window.is_visible().unwrap_or(false));
        SHOWN.fetch_add(1, Ordering::SeqCst);
        let _ = window.show();
        let _ = window.set_focus();
        let _ = window.emit(SHOWN_EVENT, ());
    }
}

/// Hides the panel after its closing animation.
pub fn hide(app: &AppHandle) {
    let Some(window) = window(app) else { return };
    if !window.is_visible().unwrap_or(false) {
        return;
    }
    let shown = SHOWN.load(Ordering::SeqCst);
    eprintln!("[overlay] hide");
    let _ = window.emit(HIDING_EVENT, ());
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(CLOSE_ANIMATION).await;
        if SHOWN.load(Ordering::SeqCst) == shown {
            let _ = window.hide();
        }
    });
}

/// Hides the panel at once, for a screenshot of the game behind it.
pub fn hide_now(app: &AppHandle) {
    if let Some(window) = window(app) {
        let _ = window.hide();
    }
}

pub fn is_visible(app: &AppHandle) -> bool {
    window(app).is_some_and(|window| window.is_visible().unwrap_or(false))
}

pub fn toggle(app: &AppHandle) {
    match window(app) {
        Some(window) if window.is_visible().unwrap_or(false) => hide(app),
        Some(_) => show(app),
        None => {}
    }
}

#[cfg(target_os = "linux")]
pub fn is_wayland() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some()
}

#[cfg(not(target_os = "linux"))]
pub fn is_wayland() -> bool {
    false
}

pub fn setup(app: &AppHandle) {
    let Some(window) = window(app) else { return };

    #[cfg(target_os = "linux")]
    if is_wayland() {
        make_layer_surface(&window);
    }

    let handle = app.clone();
    let wayland = is_wayland();
    window.on_window_event(move |event| match event {
        WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            hide(&handle);
        }
        // Clicking back into the game hides the overlay. Not on Wayland: the
        // layer surface holds keyboard focus exclusively while shown.
        WindowEvent::Focused(false) if !wayland => hide(&handle),
        _ => {}
    });
}

/// Must run before the surface is first mapped. Tauri realizes the window
/// on creation, so unrealize it first.
#[cfg(target_os = "linux")]
fn make_layer_surface(window: &WebviewWindow) {
    use gtk::prelude::*;
    use gtk_layer_shell::{KeyboardMode, Layer, LayerShell};

    let Ok(gtk_window) = window.gtk_window() else { return };
    if gtk_window.is_realized() {
        gtk_window.unrealize();
    }
    gtk_window.init_layer_shell();
    gtk_window.set_namespace("quest-backlog-overlay");
    gtk_window.set_layer(Layer::Overlay);
    gtk_window.set_keyboard_mode(KeyboardMode::Exclusive);
}

/// Wayland: hands keyboard focus back to the window below while the
/// overlay stays on screen (`false`), or takes it again (`true`). KWin then
/// treats the game as the active window, so it can be captured without
/// hiding the overlay.
#[cfg(target_os = "linux")]
pub fn set_keyboard_exclusive(app: &AppHandle, exclusive: bool) {
    use gtk_layer_shell::{KeyboardMode, LayerShell};

    let Some(window) = window(app) else { return };
    let target = window.clone();
    let _ = window.run_on_main_thread(move || {
        if let Ok(gtk_window) = target.gtk_window() {
            gtk_window.set_keyboard_mode(if exclusive { KeyboardMode::Exclusive } else { KeyboardMode::None });
            if exclusive {
                use gtk::prelude::GtkWindowExt;
                gtk_window.present();
            }
        }
    });
}

/// The overlay's size in physical pixels.
pub fn physical_size(app: &AppHandle) -> Option<(u32, u32)> {
    let size = window(app)?.inner_size().ok()?;
    Some((size.width, size.height))
}
