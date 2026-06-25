#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod capture;
mod config;
mod d3d;
mod events;
mod flashlight;
mod hotkey;
mod overlay;
mod settings;
mod tray;
mod wgc;

use std::thread;
use std::time::{Duration, Instant};

use app::App;
use config::Config;
use events::AppEvent;
use overlay::FRAME;
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, MSG, PM_REMOVE, PeekMessageW, TranslateMessage, WM_QUIT,
};

fn pump_messages() {
    unsafe {
        let mut msg = MSG::default();
        while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
            if msg.message == WM_QUIT {
                std::process::exit(0);
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

fn main() {
    capture::set_dpi_aware();

    let (config, config_path) = match Config::load_or_create() {
        Ok(v) => v,
        Err(err) => {
            eprintln!("config error: {err}");
            std::process::exit(1);
        }
    };

    let (tx, rx) = std::sync::mpsc::channel();
    tray::install_tray_handlers(tx.clone());
    hotkey::install_hotkey_handler(tx);

    let mut app = App::new(config, config_path);
    if let Err(err) = app.init() {
        eprintln!("init failed: {err}");
        std::process::exit(1);
    }

    let idle = Duration::from_millis(50);
    while !app.quit {
        while let Ok(event) = rx.try_recv() {
            match event {
                AppEvent::Hotkey(hotkey) => app.handle_hotkey(&hotkey),
                AppEvent::Menu(menu) => app.handle_menu(&menu),
            }
        }

        if app.overlay_active() {
            let frame_start = Instant::now();
            let dt = app.frame_dt();
            app.tick(dt);
            pump_messages();
            if let Some(remaining) = FRAME.checked_sub(frame_start.elapsed()) {
                thread::sleep(remaining);
            }
        } else {
            pump_messages();
            thread::sleep(idle);
        }
    }
}
