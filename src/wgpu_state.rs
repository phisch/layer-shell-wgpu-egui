use std::ptr::NonNull;

use raw_window_handle::{RawDisplayHandle, RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle};
use wayland_backend::client::Backend;
use wayland_client::{protocol::wl_surface::WlSurface, Proxy};
use wgpu::{Backends, Device, Instance, InstanceDescriptor, PresentMode, Queue, RequestAdapterOptions, Surface, SurfaceConfiguration, SurfaceTargetUnsafe, TextureFormat, TextureUsages};

use crate::Error;

pub struct WgpuState {
    pub device: Device,
    pub surface_configuration: SurfaceConfiguration,
    pub queue: Queue,
    pub surface: Surface<'static>,
}

impl WgpuState {
    pub fn new(backend: &Backend, wl_surface: &WlSurface) -> Result<Self, Error> {
        let instance = Instance::new(InstanceDescriptor {
            backends: Backends::all(),
            ..Default::default()
        });

        let raw_display_handle = RawDisplayHandle::Wayland(WaylandDisplayHandle::new(
            NonNull::new(backend.display_ptr() as *mut _).unwrap(),
        ));
        let raw_window_handle = RawWindowHandle::Wayland(WaylandWindowHandle::new(
            NonNull::new(wl_surface.id().as_ptr() as *mut _).unwrap(),
        ));

        let surface = unsafe {
            instance
                .create_surface_unsafe(SurfaceTargetUnsafe::RawHandle {
                    raw_display_handle,
                    raw_window_handle,
                })
                .unwrap()
        };

        let adapter = pollster::block_on(instance.request_adapter(&RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .expect("Failed to find suitable adapter");

        let (device, queue) = pollster::block_on(adapter.request_device(&Default::default(), None))
            .expect("Failed to request device");

        let surface_capabilities = surface.get_capabilities(&adapter);
        let texture_format = surface_capabilities
            .formats
            .iter()
            .find(|d| **d == TextureFormat::Bgra8UnormSrgb)
            .expect("failed to select proper surface texture format!");

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