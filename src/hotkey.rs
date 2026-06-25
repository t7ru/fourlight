use global_hotkey::GlobalHotKeyEvent;
use global_hotkey::GlobalHotKeyManager;
use global_hotkey::hotkey::HotKey;

use crate::events::{AppEvent, EventSender};

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
