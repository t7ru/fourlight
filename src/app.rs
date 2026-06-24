use std::path::PathBuf;
use std::time::Instant;

use tray_icon::menu::MenuEvent;

use crate::config::Config;
use crate::hotkey::{self, HotkeyManager};
use crate::overlay::LiveOverlay;
use crate::settings;
use crate::tray::Tray;

pub struct App {
    pub config: Config,
    pub config_path: PathBuf,
    overlay: Option<LiveOverlay>,
    tray: Option<Tray>,
    hotkeys: Option<HotkeyManager>,
    last_frame: Instant,
    pub quit: bool,
}

impl App {
    pub fn new(config: Config, config_path: PathBuf) -> Self {
        Self {
            config,
            config_path,
            overlay: None,
            tray: None,
            hotkeys: None,
            last_frame: Instant::now(),
            quit: false,
        }
    }

    pub fn init(&mut self) -> Result<(), String> {
        self.tray = Some(Tray::new()?);
        let mut hotkeys = HotkeyManager::new()?;
        hotkeys.register(self.config.to_hotkey()?)?;
        self.hotkeys = Some(hotkeys);
        Ok(())
    }

    pub fn overlay_active(&self) -> bool {
        self.overlay.as_ref().is_some_and(|o| o.is_active())
    }

    pub fn tick(&mut self, dt: f32) {
        if let Some(overlay) = &mut self.overlay {
            overlay.tick(dt);
        }
    }

    pub fn frame_dt(&mut self) -> f32 {
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f32().min(0.05);
        self.last_frame = now;
        dt
    }

    pub fn toggle_overlay(&mut self) {
        if self.overlay.is_none() {
            match LiveOverlay::new() {
                Ok(mut overlay) => {
                    overlay.sync_user_data();
                    overlay.show(self.config.zoom.default_zoom, &self.config.flashlight);
                    self.overlay = Some(overlay);
                    self.last_frame = Instant::now();
                    if let Some(tray) = &self.tray {
                        tray.set_tooltip("fourlight — zoom active");
                    }
                }
                Err(err) => {
                    eprintln!("overlay failed: {err}");
                    if let Some(tray) = &self.tray {
                        tray.set_tooltip(&format!("overlay failed: {err}"));
                    }
                }
            }
            return;
        }

        let Some(overlay) = &mut self.overlay else { return };
        if overlay.is_closing() {
            overlay.cancel_hide(self.config.zoom.default_zoom, &self.config.flashlight);
            if let Some(tray) = &self.tray {
                tray.set_tooltip("fourlight — zoom active");
            }
        } else if overlay.is_active() {
            overlay.begin_hide();
            if let Some(tray) = &self.tray {
                tray.set_tooltip("fourlight");
            }
        } else {
            overlay.show(self.config.zoom.default_zoom, &self.config.flashlight);
            self.last_frame = Instant::now();
            if let Some(tray) = &self.tray {
                tray.set_tooltip("fourlight — zoom active");
            }
        }
    }

    pub fn open_settings(&mut self) {
        let Some(draft) = settings::run(&self.config) else {
            return;
        };
        if let Err(err) = draft.save(&self.config_path) {
            eprintln!("save failed: {err}");
            return;
        }
        self.config = draft;
        if let Some(hk) = &mut self.hotkeys {
            if let Err(err) = hk.register(self.config.to_hotkey().unwrap()) {
                eprintln!("hotkey register failed: {err}");
            }
        }
    }

    pub fn handle_menu(&mut self, event: &MenuEvent) {
        let Some(tray) = &self.tray else { return };
        if event.id == tray.settings_id.id() {
            self.open_settings();
        } else if event.id == tray.quit_id.id() {
            settings::close_if_open();
            self.quit = true;
        }
    }

    pub fn handle_hotkey(&mut self, event: &global_hotkey::GlobalHotKeyEvent) {
        if !hotkey::is_hotkey_pressed(event) {
            return;
        }
        self.toggle_overlay();
    }
}
