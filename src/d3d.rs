use windows::Win32::Foundation::{HMODULE, HWND};
use windows::Win32::Graphics::Direct3D::Fxc::D3DCompile;
use windows::Win32::Graphics::Direct3D::{
    D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL, D3D_FEATURE_LEVEL_11_0, ID3DBlob,
};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_CONSTANT_BUFFER, D3D11_BUFFER_DESC, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
    D3D11_SAMPLER_DESC, D3D11_SDK_VERSION, D3D11_TEXTURE_ADDRESS_CLAMP, D3D11_USAGE_DEFAULT,
    D3D11_VIEWPORT, D3D11CreateDevice, ID3D11Buffer, ID3D11Device, ID3D11DeviceContext,
    ID3D11PixelShader, ID3D11RenderTargetView, ID3D11SamplerState, ID3D11ShaderResourceView,
    ID3D11Texture2D, ID3D11VertexShader,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_ALPHA_MODE_IGNORE, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory2, DXGI_CREATE_FACTORY_FLAGS, DXGI_PRESENT, DXGI_SCALING_STRETCH,
    DXGI_SWAP_CHAIN_DESC1, DXGI_SWAP_CHAIN_FLAG, DXGI_SWAP_EFFECT_FLIP_DISCARD,
    DXGI_USAGE_RENDER_TARGET_OUTPUT, IDXGIFactory2, IDXGISwapChain1,
};
use windows::core::s;

const SHADER: &[u8] = br#"
Texture2D tex0 : register(t0);
SamplerState samp0 : register(s0);

cbuffer Params : register(b0) {
    float2 screen;
    float2 source;
    float2 cursor;
    float zoom;
    float radius;
    float shadow;
    float flashlight;
};

struct VsOut {
    float4 pos : SV_Position;
    float2 uv : TEXCOORD0;
};

VsOut vs(uint id : SV_VertexID) {
    float2 p = float2((id << 1) & 2, id & 2);
    VsOut o;
    o.pos = float4(p * float2(2, -2) + float2(-1, 1), 0, 1);
    o.uv = p;
    return o;
}

float4 ps(VsOut i) : SV_Target {
    float2 px = i.uv * screen;
    float2 samplePx = (px - cursor) / zoom + cursor;
    float4 c = tex0.Sample(samp0, samplePx / source);
    float d = distance(i.uv * screen, cursor);
    float outside = step(radius * zoom, d) * flashlight;
    c.rgb = lerp(c.rgb, 0.0.xxx, outside * shadow);
    return c;
}
"#;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct ShaderParams {
    pub screen: [f32; 2],
    pub source: [f32; 2],
    pub cursor: [f32; 2],
    pub zoom: f32,
    pub radius: f32,
    pub shadow: f32,
    pub flashlight: f32,
    pub _pad: [f32; 2],
}

pub struct D3d {
    pub device: ID3D11Device,
    context: ID3D11DeviceContext,
    swapchain: IDXGISwapChain1,
    rtv: ID3D11RenderTargetView,
    vs: ID3D11VertexShader,
    ps: ID3D11PixelShader,
    sampler: ID3D11SamplerState,
    params: ID3D11Buffer,
    width: u32,
    height: u32,
}

impl D3d {
    pub fn new(hwnd: HWND, width: u32, height: u32) -> Result<Self, String> {
        unsafe {
            let mut device = None;
            let mut context = None;
            let mut feature = D3D_FEATURE_LEVEL::default();
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                Some(&[D3D_FEATURE_LEVEL_11_0]),
                D3D11_SDK_VERSION,
                Some(&mut device),
                Some(&mut feature),
                Some(&mut context),
            )
            .map_err(|e| format!("D3D11CreateDevice: {e}"))?;
            let device = device.unwrap();
            let context = context.unwrap();

            let factory: IDXGIFactory2 = CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS(0))
                .map_err(|e| format!("CreateDXGIFactory2: {e}"))?;
            let desc = DXGI_SWAP_CHAIN_DESC1 {
                Width: width,
                Height: height,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                Stereo: false.into(),
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                BufferCount: 2,
                Scaling: DXGI_SCALING_STRETCH,
                SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
                AlphaMode: DXGI_ALPHA_MODE_IGNORE,
                ..Default::default()
            };
            let swapchain = factory
                .CreateSwapChainForHwnd(&device, hwnd, &desc, None, None)
                .map_err(|e| format!("CreateSwapChainForHwnd: {e}"))?;

            let (vs_blob, ps_blob) = compile_shaders()?;
            let mut vs = None;
            device
                .CreateVertexShader(blob_bytes(&vs_blob), None, Some(&mut vs))
                .map_err(|e| format!("CreateVertexShader: {e}"))?;
            let mut ps = None;
            device
                .CreatePixelShader(blob_bytes(&ps_blob), None, Some(&mut ps))
                .map_err(|e| format!("CreatePixelShader: {e}"))?;

            let sampler_desc = D3D11_SAMPLER_DESC {
                Filter: windows::Win32::Graphics::Direct3D11::D3D11_FILTER_MIN_MAG_MIP_LINEAR,
                AddressU: D3D11_TEXTURE_ADDRESS_CLAMP,
                AddressV: D3D11_TEXTURE_ADDRESS_CLAMP,
                AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
                MaxLOD: f32::MAX,
                ..Default::default()
            };
            let mut sampler = None;
            device
                .CreateSamplerState(&sampler_desc, Some(&mut sampler))
                .map_err(|e| format!("CreateSamplerState: {e}"))?;

            let cb_desc = D3D11_BUFFER_DESC {
                ByteWidth: std::mem::size_of::<ShaderParams>() as u32,
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
                ..Default::default()
            };
            let mut params = None;
            device
                .CreateBuffer(&cb_desc, None, Some(&mut params))
                .map_err(|e| format!("CreateBuffer(params): {e}"))?;

            let rtv = create_rtv(&device, &swapchain)?;
            Ok(Self {
                device,
                context,
                swapchain,
                rtv,
                vs: vs.unwrap(),
                ps: ps.unwrap(),
                sampler: sampler.unwrap(),
                params: params.unwrap(),
                width,
                height,
            })
        }
    }

    pub fn new_shared(hwnd: HWND, width: u32, height: u32, shared: &Self) -> Result<Self, String> {
        unsafe {
            let swapchain = create_swapchain(&shared.device, hwnd, width, height)?;
            let rtv = create_rtv(&shared.device, &swapchain)?;
            let cb_desc = D3D11_BUFFER_DESC {
                ByteWidth: std::mem::size_of::<ShaderParams>() as u32,
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
                ..Default::default()
            };
            let mut params = None;
            shared
                .device
                .CreateBuffer(&cb_desc, None, Some(&mut params))
                .map_err(|e| format!("CreateBuffer(params): {e}"))?;
            Ok(Self {
                device: shared.device.clone(),
                context: shared.context.clone(),
                swapchain,
                rtv,
                vs: shared.vs.clone(),
                ps: shared.ps.clone(),
                sampler: shared.sampler.clone(),
                params: params.unwrap(),
                width,
                height,
            })
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), String> {
        if width == self.width && height == self.height {
            return Ok(());
        }
        unsafe {
            self.context.OMSetRenderTargets(None, None);
            self.swapchain
                .ResizeBuffers(
                    0,
                    width,
                    height,
                    DXGI_FORMAT_B8G8R8A8_UNORM,
                    DXGI_SWAP_CHAIN_FLAG(0),
                )
                .map_err(|e| format!("ResizeBuffers: {e}"))?;
            self.rtv = create_rtv(&self.device, &self.swapchain)?;
        }
        self.width = width;
        self.height = height;
        Ok(())
    }

    pub fn prepare_render(&self, srv: &ID3D11ShaderResourceView) {
        unsafe {
            let viewport = D3D11_VIEWPORT {
                TopLeftX: 0.0,
                TopLeftY: 0.0,
                Width: self.width as f32,
                Height: self.height as f32,
                MinDepth: 0.0,
                MaxDepth: 1.0,
            };
            self.context.RSSetViewports(Some(&[viewport]));
            self.context.IASetPrimitiveTopology(
                windows::Win32::Graphics::Direct3D::D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST,
            );
            self.context.VSSetShader(&self.vs, None);
            self.context.PSSetShader(&self.ps, None);
            self.context
                .PSSetSamplers(0, Some(&[Some(self.sampler.clone())]));
            self.context
                .PSSetShaderResources(0, Some(&[Some(srv.clone())]));
        }
    }

    pub fn draw(&self, params: ShaderParams, sync_interval: u32) -> Result<(), String> {
        unsafe {
            self.context
                .OMSetRenderTargets(Some(&[Some(self.rtv.clone())]), None);
            self.context.UpdateSubresource(
                &self.params,
                0,
                None,
                (&params as *const ShaderParams).cast(),
                0,
                0,
            );
            self.context
                .PSSetConstantBuffers(0, Some(&[Some(self.params.clone())]));
            self.context.Draw(3, 0);
            self.swapchain
                .Present(sync_interval, DXGI_PRESENT(0))
                .ok()
                .map_err(|e| format!("Present: {e}"))?;
        }
        Ok(())
    }

    pub fn finish_render(&self) {
        unsafe {
            self.context.OMSetRenderTargets(None, None);
            self.context.PSSetShaderResources(0, Some(&[None]));
        }
    }

    pub fn create_srv(
        &self,
        texture: &ID3D11Texture2D,
    ) -> Result<ID3D11ShaderResourceView, String> {
        unsafe {
            let mut srv = None;
            self.device
                .CreateShaderResourceView(texture, None, Some(&mut srv))
                .map_err(|e| format!("CreateShaderResourceView: {e}"))?;
            Ok(srv.unwrap())
        }
    }
}

#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn create_swapchain(
    device: &ID3D11Device,
    hwnd: HWND,
    width: u32,
    height: u32,
) -> Result<IDXGISwapChain1, String> {
    let factory: IDXGIFactory2 = CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS(0))
        .map_err(|e| format!("CreateDXGIFactory2: {e}"))?;
    let desc = DXGI_SWAP_CHAIN_DESC1 {
        Width: width,
        Height: height,
        Format: DXGI_FORMAT_B8G8R8A8_UNORM,
        Stereo: false.into(),
        SampleDesc: DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
        BufferCount: 2,
        Scaling: DXGI_SCALING_STRETCH,
        SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
        AlphaMode: DXGI_ALPHA_MODE_IGNORE,
        ..Default::default()
    };
    factory
        .CreateSwapChainForHwnd(device, hwnd, &desc, None, None)
        .map_err(|e| format!("CreateSwapChainForHwnd: {e}"))
}

#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn create_rtv(
    device: &ID3D11Device,
    swapchain: &IDXGISwapChain1,
) -> Result<ID3D11RenderTargetView, String> {
    let backbuffer: ID3D11Texture2D = swapchain
        .GetBuffer(0)
        .map_err(|e| format!("GetBuffer(backbuffer): {e}"))?;
    let mut rtv = None;
    device
        .CreateRenderTargetView(&backbuffer, None, Some(&mut rtv))
        .map_err(|e| format!("CreateRenderTargetView: {e}"))?;
    Ok(rtv.unwrap())
}

#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn compile_shaders() -> Result<(ID3DBlob, ID3DBlob), String> {
    let mut vs = None;
    let mut ps = None;
    let mut err = None;
    D3DCompile(
        SHADER.as_ptr().cast(),
        SHADER.len(),
        s!("fourlight.hlsl"),
        None,
        None,
        s!("vs"),
        s!("vs_5_0"),
        0,
        0,
        &mut vs,
        Some(&mut err),
    )
    .map_err(|e| shader_error("vertex shader", e, err))?;

    let mut err = None;
    D3DCompile(
        SHADER.as_ptr().cast(),
        SHADER.len(),
        s!("fourlight.hlsl"),
        None,
        None,
        s!("ps"),
        s!("ps_5_0"),
        0,
        0,
        &mut ps,
        Some(&mut err),
    )
    .map_err(|e| shader_error("pixel shader", e, err))?;

    Ok((vs.unwrap(), ps.unwrap()))
}

#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn blob_bytes(blob: &ID3DBlob) -> &[u8] {
    std::slice::from_raw_parts(blob.GetBufferPointer().cast(), blob.GetBufferSize())
}

#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn shader_error(stage: &str, e: windows::core::Error, err: Option<ID3DBlob>) -> String {
    let msg = err
        .map(|b| String::from_utf8_lossy(blob_bytes(&b)).into_owned())
        .unwrap_or_default();
    format!("compile {stage}: {e}\n{msg}")
}
