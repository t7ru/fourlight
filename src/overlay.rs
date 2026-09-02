use std::mem::size_of;
use std::sync::Once;

use crate::config::FlashlightConfig;
use crate::d3d::{D3d, ShaderParams};
use crate::flashlight::Flashlight;
use crate::wgc::WgcCapture;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Direct3D11::{ID3D11ShaderResourceView, ID3D11Texture2D};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, HBRUSH, HMONITOR, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_CONTROL, VK_F};
use windows::Win32::UI::Input::{
    GetRawInputData, HRAWINPUT, RAWINPUT, RAWINPUTDEVICE, RAWINPUTHEADER, RID_INPUT,
    RIDEV_INPUTSINK, RIM_TYPEMOUSE, RegisterRawInputDevices,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DestroyWindow, GWLP_USERDATA,
    GetCursorPos, GetSystemMetrics, GetWindowLongPtrW, HTCLIENT, HWND_TOPMOST, IDC_ARROW,
    LoadCursorW, RegisterClassW, SM_CXSCREEN, SM_CYSCREEN, SW_HIDE, SW_SHOW, SW_SHOWNA, SetCursor,
    SetForegroundWindow, SetWindowDisplayAffinity, SetWindowLongPtrW, SetWindowPos, ShowCursor,
    ShowWindow, WDA_EXCLUDEFROMCAPTURE, WM_INPUT, WM_KEYDOWN, WM_MOUSEWHEEL, WM_SETCURSOR,
    WNDCLASSW, WS_CLIPSIBLINGS, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
};
use windows::core::{Interface, w};

pub const MIN_ZOOM: f32 = 1.0;
pub const MAX_ZOOM: f32 = 16.0;
pub const FRAME: std::time::Duration = std::time::Duration::from_millis(16);
const ZOOM_STEP: f32 = 1.25;
const ZOOM_LERP: f32 = 16.0;

static REGISTER_CLASS: Once = Once::new();

struct ObsOutput {
    hwnd: HWND,
    d3d: D3d,
}

pub struct ObsOutputWindow {
    hwnd: HWND,
}

impl ObsOutputWindow {
    pub fn new() -> Result<Self, String> {
        register_class();
        unsafe {
            let instance = GetModuleHandleW(None).map_err(|e| e.to_string())?;
            let monitor = MonitorFromPoint(cursor_point(), MONITOR_DEFAULTTONEAREST);
            let rect = monitor_rect(monitor);
            let width = rect.right - rect.left;
            let height = rect.bottom - rect.top;
            let hwnd = CreateWindowExW(
                WS_EX_NOACTIVATE,
                w!("FourlightObsOutput"),
                w!("fourlight OBS output"),
                WS_POPUP | WS_CLIPSIBLINGS,
                rect.left - width,
                rect.top,
                width,
                height,
                None,
                None,
                Some(instance.into()),
                None,
            )
            .map_err(|e| format!("OBS output window: {e}"))?;
            let _ = ShowWindow(hwnd, SW_SHOWNA);
            Ok(Self { hwnd })
        }
    }

    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }
}

impl Drop for ObsOutputWindow {
    fn drop(&mut self) {
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

pub struct LiveOverlay {
    hwnd: HWND,
    rect: RECT,
    d3d: D3d,
    capture: WgcCapture,
    last_srv: Option<ID3D11ShaderResourceView>,
    srv_cache: [Option<(usize, ID3D11ShaderResourceView)>; 2],
    srv_generation: u64,
    zoom: f32,
    target_zoom: f32,
    visible: bool,
    closing: bool,
    raw_delta: [f32; 2],
    overshoot: [f32; 2],
    free_roam: bool,
    flashlight: Flashlight,
    obs_output: Option<ObsOutput>,
    obs_enabled: bool,
}

impl LiveOverlay {
    pub fn new(obs_hwnd: Option<HWND>) -> Result<Self, String> {
        register_class();
        unsafe {
            let instance = GetModuleHandleW(None).map_err(|e| e.to_string())?;
            let cursor = cursor_point();
            let monitor = MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST);
            let rect = monitor_rect(monitor);
            let width = (rect.right - rect.left) as u32;
            let height = (rect.bottom - rect.top) as u32;
            let hwnd = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                w!("FourlightOverlay"),
                w!("fourlight"),
                WS_POPUP | WS_CLIPSIBLINGS,
                rect.left,
                rect.top,
                width as i32,
                height as i32,
                None,
                None,
                Some(instance.into()),
                None,
            )
            .map_err(|e| format!("overlay window: {e}"))?;
            SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE)
                .map_err(|e| format!("SetWindowDisplayAffinity: {e}"))?;
            register_raw_mouse(hwnd)?;
            let d3d = D3d::new(hwnd, width, height)?;
            let capture = WgcCapture::new(&d3d.device, monitor)?;
            let obs_output = if let Some(hwnd) = obs_hwnd {
                Some(create_obs_output(&d3d, rect, hwnd)?)
            } else {
                None
            };
            let obs_enabled = obs_output.is_some();

            Ok(Self {
                hwnd,
                rect,
                d3d,
                capture,
                last_srv: None,
                srv_cache: [None, None],
                srv_generation: 0,
                zoom: MIN_ZOOM,
                target_zoom: MIN_ZOOM,
                visible: false,
                closing: false,
                raw_delta: [0.0, 0.0],
                overshoot: [0.0, 0.0],
                free_roam: false,
                flashlight: Flashlight::from_config(&FlashlightConfig::default()),
                obs_output,
                obs_enabled,
            })
        }
    }

    pub fn is_active(&self) -> bool {
        self.visible || self.closing
    }

    pub fn should_tick(&self) -> bool {
        self.is_active() || self.obs_enabled
    }

    pub fn is_closing(&self) -> bool {
        self.closing
    }

    pub fn set_obs_output_window(&mut self, hwnd: Option<HWND>) {
        self.obs_enabled = hwnd.is_some();
        if let Some(hwnd) = hwnd {
            if self.obs_output.as_ref().is_some_and(|obs| obs.hwnd == hwnd) {
                return;
            }
            self.obs_output = None;
            match create_obs_output(&self.d3d, self.rect, hwnd) {
                Ok(output) => self.obs_output = Some(output),
                Err(err) => eprintln!("OBS output failed: {err}"),
            }
        } else {
            self.obs_output = None;
        }
    }

    pub fn show(&mut self, target_zoom: f32, fl_cfg: &FlashlightConfig, free_roam: bool) {
        self.sync_user_data();
        self.zoom = MIN_ZOOM;
        self.target_zoom = target_zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        self.free_roam = free_roam;
        self.recenter(fl_cfg);
        self.closing = false;
        unsafe {
            let _ = SetWindowPos(
                self.hwnd,
                Some(HWND_TOPMOST),
                self.rect.left,
                self.rect.top,
                self.rect.right - self.rect.left,
                self.rect.bottom - self.rect.top,
                windows::Win32::UI::WindowsAndMessaging::SWP_NOACTIVATE,
            );
            let _ = ShowWindow(self.hwnd, SW_SHOW);
            let _ = SetForegroundWindow(self.hwnd);
            let _ = ShowCursor(false);
        }
        self.visible = true;
    }

    pub fn begin_hide(&mut self) {
        if self.visible {
            self.closing = true;
            self.target_zoom = MIN_ZOOM;
            self.flashlight.active = false;
        }
    }

    pub fn cancel_hide(&mut self, target_zoom: f32, fl_cfg: &FlashlightConfig, free_roam: bool) {
        self.closing = false;
        self.target_zoom = target_zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        self.free_roam = free_roam;
        self.recenter(fl_cfg);
    }

    pub fn tick(&mut self, dt: f32) {
        if !self.should_tick() {
            return;
        }
        if let Err(err) = self.render(dt) {
            eprintln!("render failed: {err}");
            self.begin_hide();
        }
    }

    pub fn sync_user_data(&mut self) {
        unsafe {
            SetWindowLongPtrW(
                self.hwnd,
                GWLP_USERDATA,
                (self as *mut Self).cast::<()>() as isize,
            );
        }
    }

    fn render(&mut self, dt: f32) -> Result<(), String> {
        if self.is_active() {
            let t = 1.0 - (-ZOOM_LERP * dt).exp();
            self.zoom += (self.target_zoom - self.zoom) * t;
            if (self.zoom - self.target_zoom).abs() < 0.002 {
                self.zoom = self.target_zoom;
            }
        } else {
            self.zoom = MIN_ZOOM;
            self.target_zoom = MIN_ZOOM;
        }

        let tex = self.capture.latest_texture()?;
        let current_gen = self.capture.generation();
        if current_gen != self.srv_generation {
            self.srv_cache = [None, None];
            self.srv_generation = current_gen;
            self.last_srv = None;
        }
        if let Some(tex) = tex {
            self.last_srv = Some(self.srv_for_texture(&tex)?);
        }

        let size = self.capture.size();
        let width = (self.rect.right - self.rect.left) as u32;
        let height = (self.rect.bottom - self.rect.top) as u32;
        self.d3d.resize(width, height)?;
        let cursor = self.cursor_local();
        if self.last_srv.is_none() {
            return Ok(());
        }
        let offscreen = offscreen_radius(cursor, width as f32, height as f32, self.zoom);
        self.flashlight.update(dt, offscreen);
        let flashlight = self.is_active() && self.flashlight.visible(offscreen);
        let params = ShaderParams {
            screen: [width as f32, height as f32],
            source: [size.Width as f32, size.Height as f32],
            cursor,
            zoom: self.zoom,
            radius: self.flashlight.radius,
            shadow: self.flashlight.max_shadow,
            flashlight: flashlight as u8 as f32,
            ..Default::default()
        };
        let active = self.is_active();
        if active {
            self.d3d.prepare_render(&self.last_srv);
            self.d3d.draw(params, 1)?;
        }
        if self.obs_enabled {
            if let Some(obs) = &mut self.obs_output {
                obs.d3d.resize(width, height)?;
                if !active {
                    obs.d3d.prepare_render(&self.last_srv);
                }
                obs.d3d.draw(params, 0)?;
            }
        }
        self.d3d.finish_render();

        if self.closing && self.zoom <= MIN_ZOOM + 0.002 && !self.flashlight.visible(offscreen) {
            unsafe {
                let _ = ShowWindow(self.hwnd, SW_HIDE);
                let _ = ShowCursor(true);
            }
            self.visible = false;
            self.closing = false;
        }
        Ok(())
    }

    fn on_wheel(&mut self, delta: i16) {
        let ctrl = unsafe { GetAsyncKeyState(VK_CONTROL.0 as i32) as u16 & 0x8000 } != 0;
        if ctrl && self.flashlight.active {
            self.flashlight.adjust_radius(delta > 0);
        } else if delta > 0 {
            self.target_zoom = (self.target_zoom * ZOOM_STEP).clamp(MIN_ZOOM, MAX_ZOOM);
            self.closing = false;
        } else if delta < 0 {
            self.target_zoom = (self.target_zoom / ZOOM_STEP).clamp(MIN_ZOOM, MAX_ZOOM);
            self.closing = false;
        }
    }

    fn on_key(&mut self, vk: u32) {
        if vk == VK_F.0 as u32 {
            let cursor = self.cursor_local();
            let offscreen = offscreen_radius(
                cursor,
                (self.rect.right - self.rect.left) as f32,
                (self.rect.bottom - self.rect.top) as f32,
                self.zoom,
            );
            self.flashlight.toggle(offscreen);
        }
    }

    fn cursor_local(&mut self) -> [f32; 2] {
        let phys = local_cursor_point(self.rect);
        if !self.free_roam {
            return phys;
        }
        let [dx, dy] = std::mem::take(&mut self.raw_delta);
        let w = (self.rect.right - self.rect.left) as f32;
        let h = (self.rect.bottom - self.rect.top) as f32;
        pan(&mut self.overshoot[0], dx, phys[0], w);
        pan(&mut self.overshoot[1], dy, phys[1], h);
        [phys[0] + self.overshoot[0], phys[1] + self.overshoot[1]]
    }

    fn recenter(&mut self, fl_cfg: &FlashlightConfig) {
        self.raw_delta = [0.0, 0.0];
        self.overshoot = [0.0, 0.0];
        let cursor = local_cursor_point(self.rect);
        let offscreen = offscreen_radius(
            cursor,
            (self.rect.right - self.rect.left) as f32,
            (self.rect.bottom - self.rect.top) as f32,
            self.zoom,
        );
        self.flashlight.restart(fl_cfg, offscreen);
    }

    fn srv_for_texture(
        &mut self,
        tex: &ID3D11Texture2D,
    ) -> Result<ID3D11ShaderResourceView, String> {
        let key = Interface::as_raw(tex) as usize;
        for slot in &self.srv_cache {
            if let Some((k, srv)) = slot {
                if *k == key {
                    return Ok(srv.clone());
                }
            }
        }
        let srv = self.d3d.create_srv(tex)?;
        self.srv_cache[1] = self.srv_cache[0].take();
        self.srv_cache[0] = Some((key, srv.clone()));
        Ok(srv)
    }

    fn on_raw_input(&mut self, lparam: LPARAM) {
        if !self.free_roam || !self.is_active() {
            return;
        }
        let mut input = RAWINPUT::default();
        let mut size = size_of::<RAWINPUT>() as u32;
        let ok = unsafe {
            GetRawInputData(
                HRAWINPUT(lparam.0 as _),
                RID_INPUT,
                Some((&mut input as *mut RAWINPUT).cast()),
                &mut size,
                size_of::<RAWINPUTHEADER>() as u32,
            )
        };
        if ok == u32::MAX || input.header.dwType != RIM_TYPEMOUSE.0 {
            return;
        }
        let mouse = unsafe { input.data.mouse };
        self.raw_delta[0] += mouse.lLastX as f32;
        self.raw_delta[1] += mouse.lLastY as f32;
    }
}

impl Drop for LiveOverlay {
    fn drop(&mut self) {
        unsafe {
            SetWindowLongPtrW(self.hwnd, GWLP_USERDATA, 0);
            if self.visible {
                let _ = ShowCursor(true);
            }
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

fn create_obs_output(d3d: &D3d, rect: RECT, hwnd: HWND) -> Result<ObsOutput, String> {
    let width = (rect.right - rect.left) as u32;
    let height = (rect.bottom - rect.top) as u32;
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOWNA);
    }
    Ok(ObsOutput {
        hwnd,
        d3d: D3d::new_shared(hwnd, width, height, d3d)?,
    })
}

fn register_class() {
    REGISTER_CLASS.call_once(|| unsafe {
        let instance = GetModuleHandleW(None).expect("module handle");
        for (class, proc) in [
            (w!("FourlightOverlay"), overlay_wnd_proc as _),
            (w!("FourlightObsOutput"), obs_wnd_proc as _),
        ] {
            let wc = WNDCLASSW {
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(proc),
                hInstance: instance.into(),
                hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
                hbrBackground: HBRUSH(std::ptr::null_mut()),
                lpszClassName: class,
                ..Default::default()
            };
            RegisterClassW(&wc);
        }
    });
}

fn register_raw_mouse(hwnd: HWND) -> Result<(), String> {
    let device = RAWINPUTDEVICE {
        usUsagePage: 0x01,
        usUsage: 0x02,
        dwFlags: RIDEV_INPUTSINK,
        hwndTarget: hwnd,
    };
    unsafe { RegisterRawInputDevices(&[device], size_of::<RAWINPUTDEVICE>() as u32) }
        .map_err(|e| format!("RegisterRawInputDevices: {e}"))
}

#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn overlay_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_MOUSEWHEEL => {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut LiveOverlay;
            if !ptr.is_null() {
                let delta = ((wparam.0 >> 16) & 0xFFFF) as i16;
                (*ptr).on_wheel(delta);
                return LRESULT(0);
            }
        }
        WM_KEYDOWN => {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut LiveOverlay;
            if !ptr.is_null() {
                (*ptr).on_key(wparam.0 as u32);
                return LRESULT(0);
            }
        }
        WM_INPUT => {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut LiveOverlay;
            if !ptr.is_null() {
                (*ptr).on_raw_input(lparam);
            }
        }
        WM_SETCURSOR => {
            if (lparam.0 & 0xFFFF) as u32 == HTCLIENT {
                let _ = SetCursor(None);
                return LRESULT(1);
            }
        }
        _ => {}
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn obs_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

pub fn cursor_point() -> POINT {
    let mut pt = POINT::default();
    unsafe {
        let _ = GetCursorPos(&mut pt);
    }
    pt
}

fn local_cursor_point(rect: RECT) -> [f32; 2] {
    let cursor = cursor_point();
    [(cursor.x - rect.left) as f32, (cursor.y - rect.top) as f32]
}

fn pan(over: &mut f32, delta: f32, pos: f32, size: f32) {
    if delta == 0.0 {
        return;
    }
    if (delta > 0.0 && pos >= size - 1.0) || (delta < 0.0 && pos <= 1.0) {
        *over += delta;
    } else {
        let back = over.abs().min(40.0);
        *over -= over.signum() * back;
    }
}

fn monitor_rect(mon: HMONITOR) -> RECT {
    unsafe {
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        let _ = GetMonitorInfoW(mon, &mut info);
        info.rcMonitor
    }
}

fn offscreen_radius(cursor: [f32; 2], width: f32, height: f32, zoom: f32) -> f32 {
    let corners = [[0.0, 0.0], [width, 0.0], [0.0, height], [width, height]];
    corners
        .iter()
        .map(|p| {
            let dx = p[0] - cursor[0];
            let dy = p[1] - cursor[1];
            (dx * dx + dy * dy).sqrt()
        })
        .fold(0.0, f32::max)
        / zoom.max(MIN_ZOOM)
        + 16.0
}

pub fn center_window(hwnd: HWND, width: i32, height: i32) {
    unsafe {
        let sw = GetSystemMetrics(SM_CXSCREEN);
        let sh = GetSystemMetrics(SM_CYSCREEN);
        let _ = SetWindowPos(
            hwnd,
            None,
            (sw - width) / 2,
            (sh - height) / 2,
            width,
            height,
            windows::Win32::UI::WindowsAndMessaging::SWP_NOZORDER
                | windows::Win32::UI::WindowsAndMessaging::SWP_NOACTIVATE,
        );
    }
}
