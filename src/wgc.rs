use windows::Graphics::Capture::{
    Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession,
};
use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Graphics::SizeInt32;
use windows::Win32::Graphics::Direct3D11::{ID3D11Device, ID3D11Texture2D};
use windows::Win32::Graphics::Dxgi::IDXGIDevice;
use windows::Win32::Graphics::Gdi::HMONITOR;
use windows::Win32::System::WinRT::Direct3D11::{
    CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess,
};
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;
use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize};
use windows::core::{Interface, factory};

pub struct WgcCapture {
    _device: IDirect3DDevice,
    pool: Direct3D11CaptureFramePool,
    session: GraphicsCaptureSession,
    size: SizeInt32,
}

impl WgcCapture {
    pub fn new(device: &ID3D11Device, monitor: HMONITOR) -> Result<Self, String> {
        unsafe {
            let _ = RoInitialize(RO_INIT_MULTITHREADED);
            let item = capture_item_for_monitor(monitor)?;
            let size = item
                .Size()
                .map_err(|e| format!("GraphicsCaptureItem.Size: {e}"))?;
            let dxgi: IDXGIDevice = device
                .cast()
                .map_err(|e| format!("ID3D11Device->IDXGIDevice: {e}"))?;
            let inspectable = CreateDirect3D11DeviceFromDXGIDevice(&dxgi)
                .map_err(|e| format!("CreateDirect3D11DeviceFromDXGIDevice: {e}"))?;
            let d3d_device: IDirect3DDevice = inspectable
                .cast()
                .map_err(|e| format!("IInspectable->IDirect3DDevice: {e}"))?;

            let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
                &d3d_device,
                DirectXPixelFormat::B8G8R8A8UIntNormalized,
                2,
                size,
            )
            .map_err(|e| format!("Direct3D11CaptureFramePool::CreateFreeThreaded: {e}"))?;
            let session = pool
                .CreateCaptureSession(&item)
                .map_err(|e| format!("CreateCaptureSession: {e}"))?;
            let _ = session.SetIsCursorCaptureEnabled(false);
            let _ = session.SetIsBorderRequired(false);
            session
                .StartCapture()
                .map_err(|e| format!("GraphicsCaptureSession.StartCapture: {e}"))?;

            Ok(Self {
                _device: d3d_device,
                pool,
                session,
                size,
            })
        }
    }

    pub fn size(&self) -> SizeInt32 {
        self.size
    }

    pub fn latest_texture(&mut self) -> Result<Option<ID3D11Texture2D>, String> {
        let mut latest = None;
        loop {
            match self.pool.TryGetNextFrame() {
                Ok(frame) => {
                    let size = frame
                        .ContentSize()
                        .map_err(|e| format!("Direct3D11CaptureFrame.ContentSize: {e}"))?;
                    if size != self.size {
                        self.size = size;
                        self.pool
                            .Recreate(
                                &self._device,
                                DirectXPixelFormat::B8G8R8A8UIntNormalized,
                                2,
                                size,
                            )
                            .map_err(|e| format!("Direct3D11CaptureFramePool.Recreate: {e}"))?;
                    }
                    let surface = frame
                        .Surface()
                        .map_err(|e| format!("Direct3D11CaptureFrame.Surface: {e}"))?;
                    let access: IDirect3DDxgiInterfaceAccess = surface.cast().map_err(|e| {
                        format!("IDirect3DSurface->IDirect3DDxgiInterfaceAccess: {e}")
                    })?;
                    latest = Some(unsafe {
                        access.GetInterface::<ID3D11Texture2D>().map_err(|e| {
                            format!("IDirect3DDxgiInterfaceAccess.GetInterface: {e}")
                        })?
                    });
                    let _ = frame.Close();
                }
                Err(_) => break,
            }
        }
        Ok(latest)
    }
}

impl Drop for WgcCapture {
    fn drop(&mut self) {
        let _ = self.session.Close();
        let _ = self.pool.Close();
    }
}

#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn capture_item_for_monitor(monitor: HMONITOR) -> Result<GraphicsCaptureItem, String> {
    let interop = factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()
        .map_err(|e| format!("GraphicsCaptureItem interop factory: {e}"))?;
    interop
        .CreateForMonitor(monitor)
        .map_err(|e| format!("IGraphicsCaptureItemInterop.CreateForMonitor: {e}"))
}
