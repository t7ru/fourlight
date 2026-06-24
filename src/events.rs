use global_hotkey::GlobalHotKeyEvent;
use tray_icon::menu::MenuEvent;

pub enum AppEvent {
    Hotkey(GlobalHotKeyEvent),
    Menu(MenuEvent),
}

pub type EventSender = std::sync::mpsc::Sender<AppEvent>;
