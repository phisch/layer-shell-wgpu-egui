mod keyboard_handler;
mod pointer_handler;

use std::{
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};

use egui_wgpu::ScreenDescriptor;
use keyboard_handler::handle_key_press;
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_layer, delegate_output, delegate_registry, delegate_seat,
    output::{OutputHandler, OutputState},
    reexports::{calloop::LoopHandle, calloop_wayland_source::WaylandSource},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{Capability, SeatHandler, SeatState},
    shell::{
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
        WaylandSurface,
    },
};
use wayland_client::{
    globals::registry_queue_init,
    protocol::{wl_keyboard::WlKeyboard, wl_output, wl_pointer::WlPointer, wl_seat, wl_surface},
    Connection, QueueHandle,
};

use crate::{
    egui_state::{self},
    wgpu_state::WgpuState,
    App,
};

#[derive(Default)]
pub struct LayerShellOptions {
    pub layer: Option<Layer>,
    pub namespace: String,
    pub width: u32,
    pub height: u32,
    pub anchor: Option<Anchor>,
    pub keyboard_interactivity: Option<KeyboardInteractivity>,
}

pub(crate) struct WgpuLayerShellState {
    //event_loop: Arc<EventLoop<'static, Self>>,
    loop_handle: LoopHandle<'static, Self>,
    registry_state: RegistryState,
    seat_state: SeatState,
    output_state: OutputState,
    pub(crate) queue_handle: Arc<QueueHandle<Self>>,

    pub(crate) layer: LayerSurface,
    pointer: Option<WlPointer>,
    keyboard: Option<WlKeyboard>,

    pub(crate) has_frame_callback: bool,
    is_configured: bool,

    pub(crate) exit: bool,

    pub(crate) wgpu_state: WgpuState,
    pub(crate) egui_state: egui_state::State,
    pub(crate) draw_request: Arc<RwLock<Option<Instant>>>,
}

impl WgpuLayerShellState {
    pub(crate) fn new(loop_handle: LoopHandle<'static, Self>, options: LayerShellOptions) -> Self {
        let connection = Connection::connect_to_env().unwrap();
        let (global_list, event_queue) = registry_queue_init(&connection).unwrap();
        let queue_handle: Arc<QueueHandle<WgpuLayerShellState>> = Arc::new(event_queue.handle());

        WaylandSource::new(connection.clone(), event_queue)
            .insert(loop_handle.clone())
            .unwrap();

        let compositor_state = CompositorState::bind(&global_list, &queue_handle)
            .expect("wl_compositor not available");

        let wl_surface = compositor_state.create_surface(&queue_handle);

        let layer_shell =
            LayerShell::bind(&global_list, &queue_handle).expect("layer shell not available");
        let layer_surface = layer_shell.create_layer_surface(
            &queue_handle,
            wl_surface,
            options.layer.unwrap_or(Layer::Top),
            Some(options.namespace),
            None,
        );
        if let Some(anchor) = options.anchor {
            layer_surface.set_anchor(anchor);
        }
        if let Some(keyboard_interactivity) = options.keyboard_interactivity {
            layer_surface.set_keyboard_interactivity(keyboard_interactivity);
        }
        layer_surface.set_size(options.width, options.height);
        layer_surface.commit();

        let wgpu_state = WgpuState::new(&connection.backend(), layer_surface.wl_surface())
            .expect("Could not create wgpu state");

        let egui_context = egui::Context::default();

        let draw_request = Arc::new(RwLock::new(None));

        egui_context.set_request_repaint_callback({
            let draw_request = Arc::clone(&draw_request);
            move |info| {
                let mut draw_request = draw_request.write().unwrap();
                *draw_request = Some(Instant::now() + info.delay);
            }
        });

        let egui_state = egui_state::State::new(
            egui_context,
            &wgpu_state.device,
            wgpu_state.surface_configuration.format,
            None,
            1,
        );

        WgpuLayerShellState {
            loop_handle: loop_handle.clone(),
            registry_state: RegistryState::new(&global_list),
            seat_state: SeatState::new(&global_list, &queue_handle),
            output_state: OutputState::new(&global_list, &queue_handle),

            exit: false,
            layer: layer_surface,

            pointer: None,
            keyboard: None,

            has_frame_callback: false,
            is_configured: false,

            queue_handle,

            egui_state,
            wgpu_state,
            draw_request,
        }
    }

    //fn request_redraw(&self, )

    pub(crate) fn should_draw(&mut self) -> bool {
        if !self.has_frame_callback {
            return false;
        }

        if !self.egui_state.input().events.is_empty() {
            return true;
        }

        match *self.draw_request.read().unwrap() {
            Some(time) => time <= Instant::now(),
            None => false,
        }
    }

    pub(crate) fn get_timeout(&self) -> Option<Duration> {
        match *self.draw_request.read().unwrap() {
            Some(instant) => {
                if self.has_frame_callback {
                    Some(instant.duration_since(Instant::now()))
                } else {
                    None
                }
            }
            None => None,
        }
    }

    pub(crate) fn draw(&mut self, application: &mut dyn App) {
        *self.draw_request.write().unwrap() = None;
        self.has_frame_callback = false;

        let full_output = self
            .egui_state
            .process_events(|ctx| application.update(ctx));

        let surface_texture = self
            .wgpu_state
            .surface
            .get_current_texture()
            .expect("Failed to acquire next swap chain texture");

        let surface_view = surface_texture
            .texture
            .create_view(&egui_wgpu::wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .wgpu_state
            .device
            .create_command_encoder(&egui_wgpu::wgpu::CommandEncoderDescriptor { label: None });

        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [
                self.wgpu_state.surface_configuration.width,
                self.wgpu_state.surface_configuration.height,
            ],
            pixels_per_point: 1.0, // todo: figure out where to get that from
        };

        self.egui_state.draw(
            &self.wgpu_state.device,
            &self.wgpu_state.queue,
            &mut encoder,
            &surface_view,
            screen_descriptor,
            full_output.shapes,
            full_output.textures_delta,
        );
        self.wgpu_state.queue.submit(Some(encoder.finish()));

        self.layer
            .wl_surface()
            .frame(&self.queue_handle, self.layer.wl_surface().clone());

        surface_texture.present();
    }
}

delegate_registry!(WgpuLayerShellState);
impl ProvidesRegistryState for WgpuLayerShellState {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState];
}

delegate_output!(WgpuLayerShellState);
impl OutputHandler for WgpuLayerShellState {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }
}

delegate_compositor!(WgpuLayerShellState);
impl CompositorHandler for WgpuLayerShellState {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_factor: i32,
    ) {
    }

    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_transform: wl_output::Transform,
    ) {
    }

    fn frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
        self.has_frame_callback = true;
    }

    fn surface_enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }
}

delegate_layer!(WgpuLayerShellState);
impl LayerShellHandler for WgpuLayerShellState {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {
        self.exit = true;
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        if !self.is_configured {
            self.is_configured = true;
            self.has_frame_callback = true;
            *self.draw_request.write().unwrap() = Some(Instant::now());
        }

        self.wgpu_state
            .resize(configure.new_size.0, configure.new_size.1);

        self.egui_state
            .set_size(configure.new_size.0, configure.new_size.1);
    }
}

delegate_seat!(WgpuLayerShellState);
impl SeatHandler for WgpuLayerShellState {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }

    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}

    fn new_capability(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        match capability {
            Capability::Pointer if self.pointer.is_none() => {
                let pointer = self
                    .seat_state
                    .get_pointer(qh, &seat)
                    .expect("Failed to create pointer");
                self.pointer = Some(pointer);
            }
            Capability::Keyboard if self.keyboard.is_none() => {
                self.keyboard = Some(
                    self.seat_state
                        .get_keyboard_with_repeat(
                            qh,
                            &seat,
                            None,
                            self.loop_handle.clone(),
                            Box::new(|state, _wl_kbd, event| {
                                handle_key_press(event, true, &mut state.egui_state.input());
                            }),
                        )
                        .expect("Failed to create keyboard"),
                );
            }
            _ => {}
        }
    }

    fn remove_capability(
        &mut self,
        _conn: &Connection,
        _: &QueueHandle<Self>,
        _: wl_seat::WlSeat,
        capability: Capability,
    ) {
        match capability {
            Capability::Pointer if self.pointer.is_some() => {
                self.pointer.take().unwrap().release();
            }
            Capability::Keyboard if self.keyboard.is_some() => {
                self.keyboard.take().unwrap().release();
            }
            _ => {}
        }

        if capability == Capability::Pointer && self.pointer.is_some() {
            self.pointer.take().unwrap().release();
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}
