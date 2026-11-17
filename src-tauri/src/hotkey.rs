//! Global shortcuts: one shows or hides the overlay, one takes a screenshot
//! and opens the post composer with it.
//!
//! - Windows and X11: `tauri-plugin-global-shortcut`. The shortcuts come
//!   from settings and can be changed in Preferences while the app runs.
//! - Wayland: the XDG GlobalShortcuts portal. The desktop owns the bindings:
//!   settings only give the first preferred trigger, the user changes them
//!   in the desktop's dialog (`configure`), and the triggers the desktop
//!   reports are what the UI shows.
//! - Anywhere the overlay shortcut cannot be registered, running
//!   `quest-backlog-desktop --toggle` toggles the running app, so users can
//!   bind that command in their desktop's keyboard settings instead.

use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Shortcut, ShortcutState};

use crate::settings::Settings;
use crate::{compose, overlay};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    ToggleOverlay,
    Screenshot,
}

impl Action {
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    const ALL: [Action; 2] = [Action::ToggleOverlay, Action::Screenshot];

    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    fn id(self) -> &'static str {
        match self {
            Self::ToggleOverlay => "toggle-overlay",
            Self::Screenshot => "screenshot",
        }
    }

    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    fn description(self) -> &'static str {
        match self {
            Self::ToggleOverlay => "Show or hide the Quest Backlog overlay",
            Self::Screenshot => "Take a screenshot and share it on Quest Backlog",
        }
    }
}

fn run(app: &AppHandle, action: Action) {
    match action {
        Action::ToggleOverlay => overlay::toggle(app),
        Action::Screenshot => compose::screenshot(app, crate::screenshot::Target::ActiveWindow),
    }
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HotkeyStatus {
    /// The overlay shortcut as the system reports it, e.g. "Ctrl+Shift+G".
    pub toggle: Option<String>,
    pub screenshot: Option<String>,
    /// Set when registration failed; the UI then explains `--toggle`.
    pub failed: bool,
    /// Wayland: the desktop owns the bindings; change them with `configure`.
    pub system_managed: bool,
}

#[derive(Default)]
pub struct HotkeyState {
    status: Mutex<HotkeyStatus>,
    /// Plugin path: what is registered, to dispatch presses.
    bindings: Mutex<Vec<(Shortcut, Action)>>,
    /// Portal path: asks the portal task to open the desktop's dialog.
    #[cfg(target_os = "linux")]
    configure: Mutex<Option<portal::ConfigureSender>>,
}

pub fn status(app: &AppHandle) -> HotkeyStatus {
    app.state::<HotkeyState>().status.lock().unwrap().clone()
}

fn report(app: &AppHandle, status: HotkeyStatus) {
    *app.state::<HotkeyState>().status.lock().unwrap() = status;
}

/// Parses and checks an accelerator from settings or the shortcut
/// recorder. Errors are UI message keys.
pub fn parse(accelerator: &str) -> Result<Shortcut, &'static str> {
    let shortcut: Shortcut = accelerator.trim().parse().map_err(|_| "hotkeyInvalid")?;
    let function_key = matches!(
        shortcut.key,
        Code::F1 | Code::F2 | Code::F3 | Code::F4 | Code::F5 | Code::F6 | Code::F7 | Code::F8
            | Code::F9 | Code::F10 | Code::F11 | Code::F12 | Code::F13 | Code::F14 | Code::F15
            | Code::F16 | Code::F17 | Code::F18 | Code::F19 | Code::F20 | Code::F21 | Code::F22
            | Code::F23 | Code::F24
    );
    // A bare letter would fire on every key press in every app.
    if shortcut.mods.is_empty() && !function_key {
        return Err("hotkeyNeedsModifier");
    }
    Ok(shortcut)
}

pub fn setup(app: &AppHandle, settings: &Settings) {
    #[cfg(target_os = "linux")]
    if overlay::is_wayland() {
        portal::start(app, settings);
        return;
    }

    let plugin = tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, shortcut, event| {
            if event.state() != ShortcutState::Pressed {
                return;
            }
            let action = app
                .state::<HotkeyState>()
                .bindings
                .lock()
                .unwrap()
                .iter()
                .find(|(bound, _)| bound == shortcut)
                .map(|(_, action)| *action);
            if let Some(action) = action {
                run(app, action);
            }
        })
        .build();
    if let Err(error) = app.plugin(plugin) {
        eprintln!("[hotkey] global shortcut plugin failed: {error}");
        report(app, HotkeyStatus { failed: true, ..Default::default() });
        return;
    }
    if let Err(key) = apply(app, settings) {
        eprintln!("[hotkey] could not register shortcuts from settings: {key}");
        report(app, HotkeyStatus { failed: true, ..Default::default() });
    }
}

/// Plugin path: registers the shortcuts in `settings`, replacing the
/// current ones. On failure the previous ones stay registered.
pub fn apply(app: &AppHandle, settings: &Settings) -> Result<(), &'static str> {
    let toggle = parse(&settings.hotkey)?;
    let screenshot = parse(&settings.screenshot_hotkey)?;
    if toggle == screenshot {
        return Err("hotkeySame");
    }

    let shortcuts = app.global_shortcut();
    let state = app.state::<HotkeyState>();
    let mut bindings = state.bindings.lock().unwrap();
    let previous = std::mem::take(&mut *bindings);
    let _ = shortcuts.unregister_all();

    let next = vec![(toggle, Action::ToggleOverlay), (screenshot, Action::Screenshot)];
    for (shortcut, _) in &next {
        if shortcuts.register(*shortcut).is_err() {
            let _ = shortcuts.unregister_all();
            for (shortcut, _) in &previous {
                let _ = shortcuts.register(*shortcut);
            }
            *bindings = previous;
            return Err("hotkeyTaken");
        }
    }
    *bindings = next;
    drop(bindings);

    report(
        app,
        HotkeyStatus {
            toggle: Some(settings.hotkey.trim().to_string()),
            screenshot: Some(settings.screenshot_hotkey.trim().to_string()),
            failed: false,
            system_managed: false,
        },
    );
    Ok(())
}

/// Wayland: opens the desktop's own dialog to change the shortcuts.
#[cfg(target_os = "linux")]
pub async fn configure(app: &AppHandle) -> Result<(), String> {
    let sender = app.state::<HotkeyState>().configure.lock().unwrap().clone();
    match sender {
        Some(sender) => portal::configure(sender).await,
        None => Err("hotkeySystem".to_string()),
    }
}

#[cfg(not(target_os = "linux"))]
pub async fn configure(_app: &AppHandle) -> Result<(), String> {
    Err("hotkeySystem".to_string())
}

/// "Ctrl+Shift+G" -> "CTRL+SHIFT+G", the portal's shortcut format.
#[cfg(target_os = "linux")]
fn portal_trigger(accelerator: &str) -> String {
    accelerator
        .split('+')
        .map(|part| match part.trim().to_uppercase().as_str() {
            "CONTROL" | "COMMANDORCONTROL" | "CMDORCTRL" => "CTRL".to_string(),
            "SUPER" | "META" | "CMD" | "COMMAND" => "LOGO".to_string(),
            other => other.to_string(),
        })
        .collect::<Vec<_>>()
        .join("+")
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn parse_requires_a_modifier_except_for_function_keys() {
        assert!(parse("Ctrl+Shift+G").is_ok());
        assert!(parse("Ctrl+Shift+F12").is_ok());
        assert!(parse("F9").is_ok());
        assert_eq!(parse("G").unwrap_err(), "hotkeyNeedsModifier");
        assert_eq!(parse("Ctrl+Nope").unwrap_err(), "hotkeyInvalid");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn converts_accelerators_to_portal_format() {
        use super::portal_trigger;
        assert_eq!(portal_trigger("Ctrl+Shift+G"), "CTRL+SHIFT+G");
        assert_eq!(portal_trigger("CommandOrControl+Alt+F9"), "CTRL+ALT+F9");
        assert_eq!(portal_trigger("Super + O"), "LOGO+O");
    }
}

#[cfg(target_os = "linux")]
mod portal {
    use ashpd::desktop::global_shortcuts::{GlobalShortcuts, NewShortcut, Shortcut};
    use futures_util::StreamExt;
    use tauri::{AppHandle, Manager};
    use tokio::sync::{mpsc, oneshot};

    use super::{portal_trigger, report, run, Action, HotkeyState, HotkeyStatus};
    use crate::settings::Settings;

    pub type ConfigureSender = mpsc::UnboundedSender<oneshot::Sender<Result<(), String>>>;

    pub fn start(app: &AppHandle, settings: &Settings) {
        let (sender, receiver) = mpsc::unbounded_channel();
        *app.state::<HotkeyState>().configure.lock().unwrap() = Some(sender);
        let handle = app.clone();
        let triggers = [portal_trigger(&settings.hotkey), portal_trigger(&settings.screenshot_hotkey)];
        tauri::async_runtime::spawn(async move {
            if let Err(error) = run_portal(&handle, triggers, receiver).await {
                eprintln!("[hotkey] portal shortcut failed: {error}");
                report(&handle, HotkeyStatus { failed: true, system_managed: true, ..Default::default() });
            }
        });
    }

    pub async fn configure(sender: ConfigureSender) -> Result<(), String> {
        let (reply, answer) = oneshot::channel();
        sender.send(reply).map_err(|_| "hotkeySystem".to_string())?;
        answer.await.map_err(|_| "hotkeySystem".to_string())?
    }

    fn trigger_of(shortcuts: &[Shortcut], action: Action) -> Option<String> {
        shortcuts
            .iter()
            .find(|shortcut| shortcut.id() == action.id())
            .map(|shortcut| shortcut.trigger_description().to_string())
            .filter(|description| !description.is_empty())
    }

    fn report_bound(app: &AppHandle, shortcuts: &[Shortcut]) {
        let status = HotkeyStatus {
            toggle: trigger_of(shortcuts, Action::ToggleOverlay),
            screenshot: trigger_of(shortcuts, Action::Screenshot),
            failed: false,
            system_managed: true,
        };
        eprintln!("[hotkey] portal bound {:?} / {:?}", status.toggle, status.screenshot);
        report(app, status);
    }

    async fn run_portal(
        app: &AppHandle,
        triggers: [String; 2],
        mut configure_requests: mpsc::UnboundedReceiver<oneshot::Sender<Result<(), String>>>,
    ) -> ashpd::Result<()> {
        // Lets the portal tie the bindings to our .desktop file. It fails
        // when the app is not installed (for example `pnpm tauri dev`); the
        // shortcuts still work, KDE just files them under the launching app.
        let app_id = ashpd::AppID::try_from(app.config().identifier.as_str())?;
        if let Err(error) = ashpd::register_host_app(app_id).await {
            eprintln!("[hotkey] portal app registration skipped: {error}");
        }

        let portal = GlobalShortcuts::new().await?;
        let session = portal.create_session(Default::default()).await?;
        let shortcuts: Vec<NewShortcut> = Action::ALL
            .iter()
            .zip(triggers.iter())
            .map(|(action, trigger)| {
                NewShortcut::new(action.id(), action.description()).preferred_trigger(trigger.as_str())
            })
            .collect();
        let bound = portal
            .bind_shortcuts(&session, &shortcuts, None, Default::default())
            .await?
            .response()?;
        report_bound(app, bound.shortcuts());

        let mut activated = portal.receive_activated().await?;
        let mut changed = portal.receive_shortcuts_changed().await?;
        loop {
            tokio::select! {
                event = activated.next() => {
                    let Some(event) = event else { break };
                    let action = Action::ALL.into_iter().find(|action| action.id() == event.shortcut_id());
                    if let Some(action) = action {
                        eprintln!("[hotkey] activated {}", action.id());
                        let handle = app.clone();
                        let _ = app.run_on_main_thread(move || run(&handle, action));
                    }
                }
                Some(event) = changed.next() => report_bound(app, event.shortcuts()),
                Some(reply) = configure_requests.recv() => {
                    let result = portal
                        .configure_shortcuts(&session, None, Default::default())
                        .await
                        .map_err(|error| {
                            eprintln!("[hotkey] configure shortcuts failed: {error}");
                            "hotkeySystem".to_string()
                        });
                    let _ = reply.send(result);
                }
            }
        }
        drop(session);
        Ok(())
    }
}
