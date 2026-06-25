use std::mem::size_of;
use std::sync::Once;
use std::sync::atomic::{AtomicIsize, Ordering};

use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CLIP_DEFAULT_PRECIS, COLOR_WINDOW, CreateFontW, DEFAULT_CHARSET, DEFAULT_QUALITY, DeleteObject,
    FF_DONTCARE, FONT_WEIGHT, FW_NORMAL, FW_SEMIBOLD, GetSysColorBrush, HDC, HFONT, HGDIOBJ,
    OUT_DEFAULT_PRECIS, SetBkMode, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::{
    BST_CHECKED, ICC_BAR_CLASSES, ICC_STANDARD_CLASSES, INITCOMMONCONTROLSEX, InitCommonControlsEx,
    TBM_SETPOS, TBM_SETRANGE, TBS_NOTICKS, TRACKBAR_CLASS,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    EnableWindow, GetAsyncKeyState, SetFocus, VK_CONTROL, VK_ESCAPE, VK_LWIN, VK_MENU, VK_RWIN,
    VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    BM_GETCHECK, BM_SETCHECK, BS_AUTOCHECKBOX, BS_DEFPUSHBUTTON, BS_PUSHBUTTON, CREATESTRUCTW,
    CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW,
    GWLP_USERDATA, GetMessageW, GetWindowLongPtrW, HICON, HMENU, IDC_ARROW, IMAGE_ICON, IsWindow,
    LR_DEFAULTSIZE, LoadCursorW, LoadImageW, MB_ICONERROR, MB_OK, MSG, MessageBoxW, PostMessageW,
    RegisterClassW, SW_SHOW, SendMessageW, SetWindowLongPtrW, SetWindowTextW, ShowWindow,
    TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLOSE, WM_COMMAND, WM_CREATE,
    WM_CTLCOLORBTN, WM_CTLCOLORSTATIC, WM_DESTROY, WM_HSCROLL, WM_KEYDOWN, WM_SETFONT,
    WM_SYSKEYDOWN, WNDCLASSW, WS_CAPTION, WS_CHILD, WS_OVERLAPPED, WS_SYSMENU, WS_TABSTOP,
    WS_VISIBLE,
};
use windows::core::{PCWSTR, w};

use crate::config::{Config, code_from_vk};
use crate::overlay::center_window;

static SETTINGS_HWND: AtomicIsize = AtomicIsize::new(0);

pub fn close_if_open() {
    let hwnd = SETTINGS_HWND.load(Ordering::Relaxed);
    if hwnd != 0 {
        unsafe {
            let _ = PostMessageW(Some(HWND(hwnd as _)), WM_CLOSE, WPARAM(0), LPARAM(0));
        }
    }
}

const TBM_GETPOS: u32 = 1024;
const SS_RIGHT: WINDOW_STYLE = WINDOW_STYLE(0x0002);

const ID_HOTKEY: i32 = 101;
const ID_FLASHLIGHT: i32 = 103;
const ID_ZOOM: i32 = 104;
const ID_RADIUS: i32 = 105;
const ID_SHADOW: i32 = 106;
const ID_OBS_OUTPUT: i32 = 107;
const ID_SAVE: i32 = 1;
const ID_CANCEL: i32 = 2;

const W: i32 = 600;
const H: i32 = 500;
const M: i32 = 24;
const LABEL_W: i32 = 130;
const VALUE_W: i32 = 70;
const CONTROL_W: i32 = W - M * 2 - LABEL_W - VALUE_W - 16;

struct Dialog {
    draft: Config,
    hotkey: HWND,
    recording: bool,
    flashlight: HWND,
    zoom: HWND,
    zoom_value: HWND,
    radius: HWND,
    radius_value: HWND,
    shadow: HWND,
    shadow_value: HWND,
    obs_output: HWND,
    saved: bool,
    font: HFONT,
    title_font: HFONT,
    section_font: HFONT,
}

pub fn run(initial: &Config) -> Option<Config> {
    setup_window_class();

    unsafe {
        let instance = GetModuleHandleW(None).ok()?;
        let mut dialog = Dialog {
            draft: initial.clone(),
            hotkey: HWND::default(),
            recording: false,
            flashlight: HWND::default(),
            zoom: HWND::default(),
            zoom_value: HWND::default(),
            radius: HWND::default(),
            radius_value: HWND::default(),
            shadow: HWND::default(),
            shadow_value: HWND::default(),
            obs_output: HWND::default(),
            saved: false,
            font: HFONT::default(),
            title_font: HFONT::default(),
            section_font: HFONT::default(),
        };

        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("FourlightSettings"),
            w!("fourlight settings"),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU,
            0,
            0,
            W,
            H,
            None,
            None,
            Some(instance.into()),
            Some((&mut dialog as *mut Dialog).cast()),
        )
        .ok()?;

        SETTINGS_HWND.store(hwnd.0 as isize, Ordering::Relaxed);
        center_window(hwnd, W, H);
        let _ = ShowWindow(hwnd, SW_SHOW);

        let mut msg = MSG::default();
        while IsWindow(Some(hwnd)).as_bool() && GetMessageW(&mut msg, None, 0, 0).0 > 0 {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        for font in [dialog.font, dialog.title_font, dialog.section_font] {
            let _ = DeleteObject(HGDIOBJ::from(font));
        }

        dialog.saved.then_some(dialog.draft)
    }
}

fn setup_window_class() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| unsafe {
        let _ = InitCommonControlsEx(&INITCOMMONCONTROLSEX {
            dwSize: size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_STANDARD_CLASSES | ICC_BAR_CLASSES,
        });

        let instance = GetModuleHandleW(None).expect("module handle");
        RegisterClassW(&WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            hInstance: instance.into(),
            hIcon: LoadImageW(
                Some(instance.into()),
                PCWSTR(1usize as *const u16),
                IMAGE_ICON,
                0,
                0,
                LR_DEFAULTSIZE,
            )
            .map(|h| HICON(h.0))
            .unwrap_or_default(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hbrBackground: GetSysColorBrush(COLOR_WINDOW),
            lpszClassName: w!("FourlightSettings"),
            ..Default::default()
        });
    });
}

#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_CREATE => {
            let cs = &*(lp.0 as *const CREATESTRUCTW);
            let dialog = &mut *(cs.lpCreateParams as *mut Dialog);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, dialog as *mut _ as isize);

            dialog.font = font(15, FW_NORMAL);
            dialog.title_font = font(22, FW_SEMIBOLD);
            dialog.section_font = font(16, FW_SEMIBOLD);

            build_ui(hwnd, cs.hInstance, dialog);
            LRESULT(0)
        }
        WM_CTLCOLORSTATIC | WM_CTLCOLORBTN => {
            let _ = SetBkMode(HDC(wp.0 as _), TRANSPARENT);
            LRESULT(GetSysColorBrush(COLOR_WINDOW).0 as isize)
        }
        WM_HSCROLL => {
            update_values(dialog(hwnd));
            LRESULT(0)
        }
        WM_KEYDOWN | WM_SYSKEYDOWN => {
            let dialog = dialog(hwnd);
            if dialog.recording {
                return capture_hotkey(dialog, wp.0 as u32);
            }
            DefWindowProcW(hwnd, msg, wp, lp)
        }
        WM_COMMAND => {
            let dialog = dialog(hwnd);
            match (wp.0 & 0xffff) as i32 {
                ID_SAVE => match read_dialog(dialog) {
                    Ok(()) => {
                        dialog.saved = true;
                        let _ = DestroyWindow(hwnd);
                    }
                    Err(err) => {
                        let _ = MessageBoxW(
                            Some(hwnd),
                            &windows::core::HSTRING::from(err),
                            w!("fourlight"),
                            MB_OK | MB_ICONERROR,
                        );
                    }
                },
                ID_CANCEL => {
                    let _ = DestroyWindow(hwnd);
                }
                ID_HOTKEY => start_hotkey_capture(dialog, hwnd),
                ID_FLASHLIGHT => set_flashlight_controls(dialog, checked(dialog.flashlight)),
                _ => {}
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            SETTINGS_HWND.store(0, Ordering::Relaxed);
            LRESULT(0)
        }
        WM_CLOSE => {
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn build_ui(hwnd: HWND, instance: HINSTANCE, d: &mut Dialog) {
    label(
        hwnd,
        instance,
        w!("fourlight"),
        M,
        18,
        180,
        28,
        d.title_font,
    );
    label(
        hwnd,
        instance,
        w!("A neat little flashlight"),
        M,
        48,
        320,
        20,
        d.font,
    );

    section(hwnd, instance, w!("Hotkey"), 86, d);
    row_label(hwnd, instance, w!("Shortcut"), 116, d.font);
    d.hotkey = hotkey_button(hwnd, instance, ID_HOTKEY, M + LABEL_W, 112, d.font);

    section(hwnd, instance, w!("Zoom"), 164, d);
    d.zoom_value = value(hwnd, instance, 194, d.font);
    d.zoom = slider(hwnd, instance, ID_ZOOM, 192);

    section(hwnd, instance, w!("Flashlight"), 242, d);
    d.flashlight = checkbox(
        hwnd,
        instance,
        ID_FLASHLIGHT,
        w!("Enable on zoom"),
        M + LABEL_W,
        238,
        d.font,
    );
    d.radius_value = value(hwnd, instance, 278, d.font);
    d.radius = slider(hwnd, instance, ID_RADIUS, 276);
    d.shadow_value = value(hwnd, instance, 314, d.font);
    d.shadow = slider(hwnd, instance, ID_SHADOW, 312);

    row_label(hwnd, instance, w!("Default zoom"), 196, d.font);
    row_label(hwnd, instance, w!("Radius"), 280, d.font);
    row_label(hwnd, instance, w!("Shadow"), 316, d.font);

    section(hwnd, instance, w!("OBS"), 350, d);
    d.obs_output = checkbox(
        hwnd,
        instance,
        ID_OBS_OUTPUT,
        w!("Virtual display for OBS"),
        M + LABEL_W,
        346,
        d.font,
    );

    button(
        hwnd,
        instance,
        ID_CANCEL,
        w!("Cancel"),
        W - M - 188,
        406,
        false,
        d.font,
    );
    button(
        hwnd,
        instance,
        ID_SAVE,
        w!("Save"),
        W - M - 92,
        406,
        true,
        d.font,
    );

    tb_range(d.zoom, 10, 160);
    tb_range(d.radius, 20, 2000);
    tb_range(d.shadow, 0, 100);
    tb_set(d.zoom, (d.draft.zoom.default_zoom * 10.0).round() as i32);
    tb_set(d.radius, d.draft.flashlight.radius.round() as i32);
    tb_set(d.shadow, (d.draft.flashlight.shadow * 100.0).round() as i32);

    set_text(d.hotkey, &d.draft.hotkey.display());
    set_checked(d.flashlight, d.draft.flashlight.enabled);
    set_checked(d.obs_output, d.draft.obs_output.enabled);
    update_values(d);
    set_flashlight_controls(d, d.draft.flashlight.enabled);
}

fn read_dialog(d: &mut Dialog) -> Result<(), String> {
    if d.draft.hotkey.key.is_empty() {
        return Err("set a shortcut first".into());
    }

    d.draft.zoom.default_zoom = tb_pos(d.zoom) as f32 / 10.0;
    d.draft.flashlight.enabled = checked(d.flashlight);
    d.draft.flashlight.radius = tb_pos(d.radius) as f32;
    d.draft.flashlight.shadow = tb_pos(d.shadow) as f32 / 100.0;
    d.draft.obs_output.enabled = checked(d.obs_output);
    d.draft.to_hotkey()?;
    Ok(())
}

fn update_values(d: &Dialog) {
    set_text(
        d.zoom_value,
        &format!("{:.1}x", tb_pos(d.zoom) as f32 / 10.0),
    );
    set_text(d.radius_value, &format!("{} px", tb_pos(d.radius)));
    set_text(d.shadow_value, &format!("{}%", tb_pos(d.shadow)));
}

fn set_flashlight_controls(d: &Dialog, enabled: bool) {
    unsafe {
        for hwnd in [d.radius, d.radius_value, d.shadow, d.shadow_value] {
            let _ = EnableWindow(hwnd, enabled);
        }
    }
}

fn start_hotkey_capture(d: &mut Dialog, hwnd: HWND) {
    d.recording = true;
    set_text(d.hotkey, "Press shortcut…");
    unsafe {
        let _ = SetFocus(Some(hwnd));
    }
}

fn capture_hotkey(d: &mut Dialog, vk: u32) -> LRESULT {
    if vk == VK_ESCAPE.0 as u32 {
        d.recording = false;
        set_text(d.hotkey, &d.draft.hotkey.display());
        return LRESULT(0);
    }

    if is_modifier(vk) {
        set_text(d.hotkey, &preview_hotkey(&read_modifiers(), None));
        return LRESULT(0);
    }

    let Some(key) = code_from_vk(vk) else {
        return LRESULT(0);
    };

    d.draft.hotkey.modifiers = read_modifiers();
    d.draft.hotkey.key = key.to_string();
    d.recording = false;
    set_text(d.hotkey, &d.draft.hotkey.display());
    LRESULT(0)
}

fn read_modifiers() -> Vec<String> {
    unsafe {
        let mut mods = Vec::new();
        if key_down(VK_CONTROL) {
            mods.push("CONTROL".into());
        }
        if key_down(VK_MENU) {
            mods.push("ALT".into());
        }
        if key_down(VK_SHIFT) {
            mods.push("SHIFT".into());
        }
        if key_down(VK_LWIN) || key_down(VK_RWIN) {
            mods.push("SUPER".into());
        }
        mods
    }
}

#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn key_down(vk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY) -> bool {
    GetAsyncKeyState(vk.0 as i32) as u16 & 0x8000 != 0
}

fn is_modifier(vk: u32) -> bool {
    matches!(vk, 0x10 | 0x11 | 0x12 | 0x5B | 0x5C | 0xA0..=0xA5)
}

fn preview_hotkey(mods: &[String], key: Option<&str>) -> String {
    if let Some(k) = key {
        return crate::config::HotkeyConfig {
            modifiers: mods.to_vec(),
            key: k.to_string(),
        }
        .display();
    }
    if mods.is_empty() {
        return "Press shortcut…".into();
    }
    let labels: Vec<String> = mods
        .iter()
        .map(|m| match m.as_str() {
            "CONTROL" => "Ctrl".into(),
            "ALT" => "Alt".into(),
            "SHIFT" => "Shift".into(),
            "SUPER" => "Win".into(),
            other => other.into(),
        })
        .collect();
    format!("{} + …", labels.join(" + "))
}

#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn label(
    parent: HWND,
    instance: HINSTANCE,
    text: PCWSTR,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    font: HFONT,
) -> HWND {
    let hwnd = control(
        parent,
        instance,
        w!("STATIC"),
        text,
        0,
        WINDOW_STYLE(0),
        x,
        y,
        w,
        h,
    );
    set_font(hwnd, font);
    hwnd
}

#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn section(parent: HWND, instance: HINSTANCE, text: PCWSTR, y: i32, d: &Dialog) {
    label(parent, instance, text, M, y, 180, 22, d.section_font);
}

#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn row_label(parent: HWND, instance: HINSTANCE, text: PCWSTR, y: i32, font: HFONT) {
    label(parent, instance, text, M, y, LABEL_W - 12, 22, font);
}

#[allow(unsafe_op_in_unsafe_fn)]
#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn value(parent: HWND, instance: HINSTANCE, y: i32, font: HFONT) -> HWND {
    let hwnd = control(
        parent,
        instance,
        w!("STATIC"),
        w!(""),
        0,
        SS_RIGHT,
        W - M * 2 - VALUE_W,
        y,
        VALUE_W,
        22,
    );
    set_font(hwnd, font);
    hwnd
}

#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn hotkey_button(
    parent: HWND,
    instance: HINSTANCE,
    id: i32,
    x: i32,
    y: i32,
    font: HFONT,
) -> HWND {
    let hwnd = control(
        parent,
        instance,
        w!("BUTTON"),
        w!(""),
        id,
        WINDOW_STYLE(BS_PUSHBUTTON as u32),
        x,
        y,
        CONTROL_W,
        30,
    );
    set_font(hwnd, font);
    hwnd
}

#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn slider(parent: HWND, instance: HINSTANCE, id: i32, y: i32) -> HWND {
    control(
        parent,
        instance,
        TRACKBAR_CLASS,
        w!(""),
        id,
        WINDOW_STYLE(TBS_NOTICKS as u32),
        M + LABEL_W,
        y,
        CONTROL_W,
        30,
    )
}

#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn checkbox(
    parent: HWND,
    instance: HINSTANCE,
    id: i32,
    text: PCWSTR,
    x: i32,
    y: i32,
    font: HFONT,
) -> HWND {
    let hwnd = control(
        parent,
        instance,
        w!("BUTTON"),
        text,
        id,
        WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
        x,
        y,
        180,
        24,
    );
    set_font(hwnd, font);
    hwnd
}

#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn button(
    parent: HWND,
    instance: HINSTANCE,
    id: i32,
    text: PCWSTR,
    x: i32,
    y: i32,
    default: bool,
    font: HFONT,
) {
    let style = WINDOW_STYLE(if default {
        BS_DEFPUSHBUTTON
    } else {
        BS_PUSHBUTTON
    } as u32);
    let hwnd = control(
        parent,
        instance,
        w!("BUTTON"),
        text,
        id,
        style,
        x,
        y,
        82,
        30,
    );
    set_font(hwnd, font);
}

#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn control(
    parent: HWND,
    instance: HINSTANCE,
    class: PCWSTR,
    text: PCWSTR,
    id: i32,
    style: WINDOW_STYLE,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) -> HWND {
    CreateWindowExW(
        WINDOW_EX_STYLE(0),
        class,
        text,
        WS_CHILD | WS_VISIBLE | style | if id == 0 { WINDOW_STYLE(0) } else { WS_TABSTOP },
        x,
        y,
        w,
        h,
        Some(parent),
        (id != 0).then_some(HMENU(id as isize as *mut _)),
        Some(instance),
        None,
    )
    .unwrap_or_default()
}

#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn font(height: i32, weight: FONT_WEIGHT) -> HFONT {
    CreateFontW(
        height,
        0,
        0,
        0,
        weight.0 as i32,
        0,
        0,
        0,
        DEFAULT_CHARSET,
        OUT_DEFAULT_PRECIS,
        CLIP_DEFAULT_PRECIS,
        DEFAULT_QUALITY,
        FF_DONTCARE.0 as u32,
        w!("Segoe UI"),
    )
}

fn dialog(hwnd: HWND) -> &'static mut Dialog {
    unsafe { &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Dialog) }
}

fn tb_pos(hwnd: HWND) -> i32 {
    unsafe { SendMessageW(hwnd, TBM_GETPOS, None, None).0 as i32 }
}

fn tb_range(hwnd: HWND, min: i32, max: i32) {
    unsafe {
        let range = (min & 0xffff) | ((max & 0xffff) << 16);
        let _ = SendMessageW(
            hwnd,
            TBM_SETRANGE,
            Some(WPARAM(1)),
            Some(LPARAM(range as isize)),
        );
    }
}

fn tb_set(hwnd: HWND, pos: i32) {
    unsafe {
        let _ = SendMessageW(
            hwnd,
            TBM_SETPOS,
            Some(WPARAM(1)),
            Some(LPARAM(pos as isize)),
        );
    }
}

fn set_font(hwnd: HWND, font: HFONT) {
    unsafe {
        let _ = SendMessageW(
            hwnd,
            WM_SETFONT,
            Some(WPARAM(font.0 as usize)),
            Some(LPARAM(1)),
        );
    }
}

fn set_text(hwnd: HWND, text: &str) {
    unsafe {
        let _ = SetWindowTextW(hwnd, &windows::core::HSTRING::from(text));
    }
}

fn set_checked(hwnd: HWND, checked: bool) {
    unsafe {
        let _ = SendMessageW(
            hwnd,
            BM_SETCHECK,
            Some(WPARAM(if checked { BST_CHECKED.0 as usize } else { 0 })),
            Some(LPARAM(0)),
        );
    }
}

fn checked(hwnd: HWND) -> bool {
    unsafe { SendMessageW(hwnd, BM_GETCHECK, None, None).0 as u32 == BST_CHECKED.0 }
}
