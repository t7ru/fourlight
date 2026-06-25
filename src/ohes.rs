use global_hotkey::GlobalHotKeyEvent;
use global_hotkey::GlobalHotKeyManager;
use global_hotkey::hotkey::HotKey;
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

use crate::settings;

pub enum AppEvent {
    Hotkey(GlobalHotKeyEvent),
    Menu(MenuEvent),
}

pub type EventSender = std::sync::mpsc::Sender<AppEvent>;

pub struct HotkeyManager {
    inner: GlobalHotKeyManager,
    active: Option<HotKey>,
}

impl HotkeyManager {
    pub fn new() -> Result<Self, String> {
        GlobalHotKeyManager::new()
            .map(|inner| Self {
                inner,
                active: None,
            })
            .map_err(|e| e.to_string())
    }

    pub fn register(&mut self, hotkey: HotKey) -> Result<(), String> {
        if let Some(old) = self.active.take() {
            self.inner.unregister(old).map_err(|e| e.to_string())?;
        }
        self.inner.register(hotkey).map_err(|e| e.to_string())?;
        self.active = Some(hotkey);
        Ok(())
    }
}

pub fn install_hotkey_handler(tx: EventSender) {
    GlobalHotKeyEvent::set_event_handler(Some(move |event| {
        let _ = tx.send(AppEvent::Hotkey(event));
    }));
}

pub struct Tray {
    icon: TrayIcon,
    pub settings_id: MenuItem,
    pub quit_id: MenuItem,
}

impl Tray {
    pub fn new() -> Result<Self, String> {
        let icon = make_icon();
        let settings_id = MenuItem::with_id("settings", "Settings", true, None);
        let quit_id = MenuItem::with_id("quit", "Quit", true, None);
        let menu = Menu::new();
        menu.append(&settings_id).map_err(|e| e.to_string())?;
        menu.append(&PredefinedMenuItem::separator())
            .map_err(|e| e.to_string())?;
        menu.append(&quit_id).map_err(|e| e.to_string())?;

        let icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("fourlight")
            .with_icon(icon)
            .build()
            .map_err(|e| e.to_string())?;

        Ok(Self {
            icon,
            settings_id,
            quit_id,
        })
    }

    pub fn set_tooltip(&self, text: &str) {
        let _ = self.icon.set_tooltip(Some(text));
    }
}

pub fn install_tray_handlers(tx: EventSender) {
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        if event.id.0 == "quit" {
            settings::close_if_open();
        }
        let _ = tx.send(AppEvent::Menu(event));
    }));
}

fn make_icon() -> Icon {
    let size = 16u32;
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let cx = x as f32 - size as f32 / 2.0;
            let cy = y as f32 - size as f32 / 2.0;
            let r = (cx * cx + cy * cy).sqrt();
            if r < size as f32 * 0.45 {
                rgba.extend_from_slice(&[240, 200, 80, 255]);
            } else {
                rgba.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
    }
    Icon::from_rgba(rgba, size, size).expect("tray icon")
}
