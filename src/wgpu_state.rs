use std::ptr::NonNull;

use raw_window_handle::{
    RawDisplayHandle, RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle,
};
use thiserror::Error;
use wayland_backend::client::Backend;
use wayland_client::{protocol::wl_surface::WlSurface, Proxy};
use wgpu::{
    Backends, CreateSurfaceError, Device, Instance, InstanceDescriptor, PresentMode, Queue,
    RequestAdapterOptions, RequestDeviceError, Surface, SurfaceConfiguration, SurfaceTargetUnsafe,
    TextureFormat, TextureUsages,
};

#[derive(Error, Debug)]
pub enum WgpuStateError {
    #[error("Pointer to {0} is null")]
    NullPointerError(String),
    #[error("Failed to create surface: {0}")]
    CreateSurfaceError(#[from] CreateSurfaceError),
    #[error("Failed to request adapter")]
    NoAdapterError,
    #[error("Failed to request device")]
    NoDeviceError(#[from] RequestDeviceError),
    #[error("Failed to select proper surface texture format")]
    NoTextureFormatError,
}

pub struct WgpuState {
    pub device: Device,
    pub surface_configuration: SurfaceConfiguration,
    pub queue: Queue,
    pub surface: Surface<'static>,
}

impl WgpuState {
    pub fn new(backend: &Backend, wl_surface: &WlSurface) -> Result<Self, WgpuStateError> {
        let instance = Instance::new(InstanceDescriptor {
            backends: Backends::all(),
            ..Default::default()
        });

        let raw_display_handle = RawDisplayHandle::Wayland(WaylandDisplayHandle::new(
            NonNull::new(backend.display_ptr() as *mut _).ok_or(
                WgpuStateError::NullPointerError("display of backend".to_string()),
            )?,
        ));
        let raw_window_handle = RawWindowHandle::Wayland(WaylandWindowHandle::new(
            NonNull::new(wl_surface.id().as_ptr() as *mut _).ok_or(
                WgpuStateError::NullPointerError("wl_surface id".to_string()),
            )?,
        ));

        let surface = unsafe {
            instance.create_surface_unsafe(SurfaceTargetUnsafe::RawHandle {
                raw_display_handle,
                raw_window_handle,
            })?
        };

        let adapter = pollster::block_on(instance.request_adapter(&RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .ok_or(WgpuStateError::NoAdapterError)?;

        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default(), None))?;

        let surface_capabilities = surface.get_capabilities(&adapter);
        let texture_format = surface_capabilities
            .formats
            .iter()
            .find(|d| **d == TextureFormat::Bgra8UnormSrgb)
            .ok_or(WgpuStateError::NoTextureFormatError)?;

        let surface_configuration = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: *texture_format,
            width: 600,
            height: 300,
            present_mode: PresentMode::Mailbox,
            desired_maximum_frame_latency: 2,
            alpha_mode: egui_wgpu::wgpu::CompositeAlphaMode::PreMultiplied,
            view_formats: vec![*texture_format],
        };

        surface.configure(&device, &surface_configuration);

        Ok(Self {
            device,
            surface_configuration,
            queue,
            surface,
        })
    }
}
