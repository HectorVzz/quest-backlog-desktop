//! The tray icon is the app's home, like Flameshot: left click opens the
//! quick panel, right click shows the menu. The app never sits in the
//! taskbar (Preferences does, only while its window is open).
//!
//! - Linux: `ksni`, a StatusNotifierItem that reports left clicks
//!   (libayatana's tray, which Tauri uses, only opens the menu).
//! - Windows: Tauri's tray icon.

use tauri::AppHandle;

use crate::{compose, overlay, preferences};

#[derive(Clone, Copy)]
enum Item {
    QuickPanel,
    Screenshot,
    Preferences,
    Quit,
}

fn label(item: Item, language: &str) -> &'static str {
    match (item, language) {
        (Item::QuickPanel, "es") => "Abrir panel rápido",
        (Item::QuickPanel, _) => "Open quick panel",
        (Item::Screenshot, "es") => "Capturar una ventana y publicar…",
        (Item::Screenshot, _) => "Screenshot a window and post…",
        (Item::Preferences, "es") => "Preferencias…",
        (Item::Preferences, _) => "Preferences…",
        (Item::Quit, "es") => "Salir",
        (Item::Quit, _) => "Quit",
    }
}

fn run(app: &AppHandle, item: Item) {
    match item {
        Item::QuickPanel => overlay::show(app),
        Item::Screenshot => compose::screenshot(app, crate::screenshot::Target::ChosenWindow),
        Item::Preferences => preferences::open(app),
        Item::Quit => app.exit(0),
    }
}

#[cfg(target_os = "linux")]
pub use linux::{set_language, setup};

#[cfg(target_os = "windows")]
pub use windows::{set_language, setup};

#[cfg(target_os = "linux")]
mod linux {
    use ksni::menu::{MenuItem, StandardItem};
    use ksni::TrayMethods;
    use tauri::{AppHandle, Manager};

    use super::{label, run, Item};
    use crate::overlay;

    struct QuestTray {
        app: AppHandle,
        language: &'static str,
    }

    /// The app icon as ARGB32, the format StatusNotifierItem expects.
    fn icon() -> ksni::Icon {
        let image = image::load_from_memory(include_bytes!("../icons/icon.png"))
            .expect("bundled icon")
            .into_rgba8();
        let (width, height) = image.dimensions();
        let mut data = image.into_vec();
        for pixel in data.chunks_exact_mut(4) {
            pixel.rotate_right(1);
        }
        ksni::Icon { width: width as i32, height: height as i32, data }
    }

    fn on_main(app: &AppHandle, item: Item) {
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || run(&handle, item));
    }

    impl ksni::Tray for QuestTray {
        fn id(&self) -> String {
            self.app.config().identifier.clone()
        }

        fn title(&self) -> String {
            "Quest Backlog".into()
        }

        fn icon_pixmap(&self) -> Vec<ksni::Icon> {
            vec![icon()]
        }

        fn tool_tip(&self) -> ksni::ToolTip {
            ksni::ToolTip { title: "Quest Backlog".into(), ..Default::default() }
        }

        fn activate(&mut self, _x: i32, _y: i32) {
            let handle = self.app.clone();
            let _ = self.app.run_on_main_thread(move || overlay::toggle(&handle));
        }

        fn menu(&self) -> Vec<MenuItem<Self>> {
            let entry = |item: Item| -> MenuItem<Self> {
                StandardItem {
                    label: label(item, self.language).into(),
                    activate: Box::new(move |tray: &mut Self| on_main(&tray.app, item)),
                    ..Default::default()
                }
                .into()
            };
            vec![
                entry(Item::QuickPanel),
                entry(Item::Screenshot),
                MenuItem::Separator,
                entry(Item::Preferences),
                MenuItem::Separator,
                entry(Item::Quit),
            ]
        }
    }

    struct TrayHandle(ksni::Handle<QuestTray>);

    pub fn setup(app: &AppHandle, language: &'static str) -> tauri::Result<()> {
        let tray = QuestTray { app: app.clone(), language };
        let handle = app.clone();
        tauri::async_runtime::spawn(async move {
            match tray.spawn().await {
                Ok(tray) => {
                    handle.manage(TrayHandle(tray));
                }
                Err(error) => eprintln!("[tray] no system tray available: {error}"),
            }
        });
        Ok(())
    }

    pub fn set_language(app: &AppHandle, language: &'static str) {
        let Some(tray) = app.try_state::<TrayHandle>() else { return };
        let handle = tray.0.clone();
        tauri::async_runtime::spawn(async move {
            handle.update(|tray| tray.language = language).await;
        });
    }
}

#[cfg(target_os = "windows")]
mod windows {
    use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
    use tauri::{AppHandle, Manager, Wry};

    use super::{label, run, Item};
    use crate::overlay;

    const ITEMS: [(&str, Item); 4] = [
        ("quick-panel", Item::QuickPanel),
        ("screenshot", Item::Screenshot),
        ("preferences", Item::Preferences),
        ("quit", Item::Quit),
    ];

    struct TrayItems(Vec<(MenuItem<Wry>, Item)>);

    pub fn setup(app: &AppHandle, language: &'static str) -> tauri::Result<()> {
        let items = ITEMS
            .iter()
            .map(|(id, item)| Ok((MenuItem::with_id(app, *id, label(*item, language), true, None::<&str>)?, *item)))
            .collect::<tauri::Result<Vec<_>>>()?;
        let separator = PredefinedMenuItem::separator(app)?;
        let menu = Menu::with_items(
            app,
            &[&items[0].0, &items[1].0, &separator, &items[2].0, &separator, &items[3].0],
        )?;

        let mut tray = TrayIconBuilder::with_id("main")
            .tooltip("Quest Backlog")
            .menu(&menu)
            .show_menu_on_left_click(false)
            .on_menu_event(|app, event| {
                if let Some((_, item)) = ITEMS.iter().find(|(id, _)| *id == event.id.as_ref()) {
                    run(app, *item);
                }
            })
            .on_tray_icon_event(|tray, event| {
                if let TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } = event
                {
                    overlay::toggle(tray.app_handle());
                }
            });
        if let Some(icon) = app.default_window_icon() {
            tray = tray.icon(icon.clone());
        }
        tray.build(app)?;
        app.manage(TrayItems(items));
        Ok(())
    }

    pub fn set_language(app: &AppHandle, language: &'static str) {
        let Some(items) = app.try_state::<TrayItems>() else { return };
        for (menu_item, item) in &items.0 {
            let _ = menu_item.set_text(label(*item, language));
        }
    }
}
