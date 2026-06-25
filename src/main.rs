#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod config;
mod d3d;
mod flashlight;
mod ohes;
mod overlay;
mod settings;
mod wgc;

use std::thread;
use std::time::{Duration, Instant};

use app::App;
use config::Config;
use ohes::{AppEvent, install_hotkey_handler, install_tray_handlers};
use overlay::FRAME;
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
};
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
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }

    let (config, config_path) = match Config::load_or_create() {
        Ok(v) => v,
        Err(err) => {
            eprintln!("config error: {err}");
            std::process::exit(1);
        }
    };

    let (tx, rx) = std::sync::mpsc::channel();
    install_tray_handlers(tx.clone());
    install_hotkey_handler(tx);

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

        if app.should_tick() {
            let frame_start = Instant::now();
            let dt = app.frame_dt();
            app.tick(dt);
            pump_messages();
            if let Some(remaining) = FRAME.checked_sub(frame_start.elapsed()) {
                thread::sleep(remaining);
            }
        } else {
            app.retire_idle_overlay();
            pump_messages();
            thread::sleep(idle);
        }
    }
}
