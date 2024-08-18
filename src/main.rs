mod egui_state;
mod keys;

use egui::{PointerButton, Shadow};
use egui_wgpu::{
    wgpu::{
        Adapter, Backends, Device, Instance, InstanceDescriptor, PresentMode, Queue,
        RequestAdapterOptions, Surface, SurfaceConfiguration, SurfaceTargetUnsafe, TextureFormat,
        TextureUsages,
    },
    ScreenDescriptor,
};
use raw_window_handle::{
    RawDisplayHandle, RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle,
};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_layer, delegate_output, delegate_pointer, delegate_registry,
    delegate_seat,
    output::{OutputHandler, OutputState},
    reexports::{
        calloop::{EventLoop, LoopHandle},
        calloop_wayland_source::WaylandSource,
    },
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        pointer::{
            PointerEvent,
            PointerEventKind::{self},
            PointerHandler,
        },
        Capability, SeatHandler, SeatState,
    },
    shell::{
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
        WaylandSurface,
    },
};
use std::{
    ptr::NonNull,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use wayland_client::{
    globals::registry_queue_init,
    protocol::{wl_output, wl_pointer, wl_seat, wl_surface},
    Connection, Proxy, QueueHandle,
};

fn main() {
    env_logger::init();

    let initial_width = 600;
    let initial_height = 300;

    let conn = Connection::connect_to_env().unwrap();
    let (globals, event_queue) = registry_queue_init(&conn).unwrap();
    let qh: Arc<QueueHandle<WgpuEguiLayer>> = Arc::new(event_queue.handle());

    let mut event_loop: EventLoop<WgpuEguiLayer> =
        EventLoop::try_new().expect("Could not create event loop.");

    let loop_handle = event_loop.handle();
    WaylandSource::new(conn.clone(), event_queue)
        .insert(loop_handle)
        .unwrap();

    let compositor_state =
        CompositorState::bind(&globals, &qh).expect("wl_compositor not available");

    let surface = compositor_state.create_surface(&qh);

    let layer_state = LayerShell::bind(&globals, &qh).expect("layer shell not available");
    let layer =
        layer_state.create_layer_surface(&qh, surface, Layer::Top, Some("wgpu_egui_layer"), None);
    layer.set_anchor(Anchor::TOP);
    layer.set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
    layer.set_size(initial_width, initial_height);
    layer.commit();

    // Initialize wgpu
    let instance = Instance::new(InstanceDescriptor {
        backends: Backends::all(),
        ..Default::default()
    });

    // Create the raw window handle for the surface.
    let raw_display_handle = RawDisplayHandle::Wayland(WaylandDisplayHandle::new(
        NonNull::new(conn.backend().display_ptr() as *mut _).unwrap(),
    ));
    let raw_window_handle = RawWindowHandle::Wayland(WaylandWindowHandle::new(
        NonNull::new(layer.wl_surface().id().as_ptr() as *mut _).unwrap(),
    ));

    let surface = unsafe {
        instance
            .create_surface_unsafe(SurfaceTargetUnsafe::RawHandle {
                raw_display_handle,
                raw_window_handle,
            })
            .unwrap()
    };

    // Pick a supported adapter
    let adapter = pollster::block_on(instance.request_adapter(&RequestAdapterOptions {
        compatible_surface: Some(&surface),
        ..Default::default()
    }))
    .expect("Failed to find suitable adapter");

    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default(), None))
        .expect("Failed to request device");

    let swapchain_capabilities = surface.get_capabilities(&adapter);
    let selected_format = TextureFormat::Bgra8UnormSrgb;
    let swapchain_format = swapchain_capabilities
        .formats
        .iter()
        .find(|d| **d == selected_format)
        .expect("failed to select proper surface texture format!");

    let config = SurfaceConfiguration {
        usage: TextureUsages::RENDER_ATTACHMENT,
        format: *swapchain_format,
        width: 600,
        height: 300,
        present_mode: PresentMode::Mailbox,
        desired_maximum_frame_latency: 2,
        alpha_mode: egui_wgpu::wgpu::CompositeAlphaMode::PreMultiplied,
        view_formats: vec![*swapchain_format],
    };

    surface.configure(&device, &config);

    let egui_context = egui::Context::default();

    let redraw_at = Arc::new(Mutex::new(None));
    let redraw_at_clone = redraw_at.clone();

    egui_context.set_request_repaint_callback(move |info| {
        let mut redraw_at = redraw_at_clone.lock().unwrap();
        if info.delay == Duration::ZERO {
            *redraw_at = Some(Instant::now());
        } else {
            *redraw_at = Some(Instant::now() + info.delay);
        }
    });

    let egui_state = egui_state::State::new(
        egui_context,
        egui::viewport::ViewportId::ROOT,
        &device,
        config.format,
        None,
        1,
    );

    let mut wgpu = WgpuEguiLayer {
        registry_state: RegistryState::new(&globals),
        seat_state: SeatState::new(&globals, &qh),
        output_state: OutputState::new(&globals, &qh),

        exit: false,
        width: initial_width,
        height: initial_height,
        layer,
        device,
        surface,
        adapter,
        queue,

        egui_state,
        //egui_renderer,
        wgpu_config: config,

        first_configure: true,
        loop_handle: event_loop.handle(),

        pointer: None,
        name: "foo".to_string(),

        can_draw: false,
        has_events: true,

        redraw_at: redraw_at,
    };


    loop {
        let timeout = {
            let redraw_at = wgpu.redraw_at.lock().unwrap();
            match *redraw_at {
                Some(instant) => Some(instant - Instant::now()),
                None => None,
            }
        };

        event_loop.dispatch(timeout, &mut wgpu).unwrap();

        if wgpu.should_draw() {
            wgpu.draw(&qh);
        }

        if wgpu.exit {
            println!("exiting example");
            break;
        }
    }

    // On exit we must destroy the surface before the window is destroyed.
    drop(wgpu.surface);
    drop(wgpu.layer);
}

struct WgpuEguiLayer {
    registry_state: RegistryState,
    seat_state: SeatState,
    output_state: OutputState,

    exit: bool,
    width: u32,
    height: u32,
    layer: LayerSurface,

    adapter: Adapter,
    device: Device,
    queue: Queue,
    surface: Surface<'static>,

    egui_state: egui_state::State,
    //egui_renderer: EguiRenderer,
    wgpu_config: egui_wgpu::wgpu::SurfaceConfiguration,

    first_configure: bool,

    loop_handle: LoopHandle<'static, WgpuEguiLayer>,

    pointer: Option<wl_pointer::WlPointer>,

    name: String,

    can_draw: bool,
    has_events: bool,

    redraw_at: Arc<Mutex<Option<Instant>>>,
}

impl WgpuEguiLayer {
    pub fn should_draw(&self) -> bool {
        let redraw_at = self.redraw_at.lock().unwrap();

        if !self.can_draw {
            return false;
        }

        if self.has_events {
            return true;
        }

        match redraw_at.as_ref() {
            Some(instant) => {
                if Instant::now() >= *instant {
                    return true;
                }
            }
            None => return false,
        }

        return false;
    }

    pub fn draw(&mut self, qh: &QueueHandle<Self>) {
        self.can_draw = false;
        self.has_events = false;
        *self.redraw_at.lock().unwrap() = None;

        let full_output = self.egui_state.process_events(
            |ctx| {
                let my_frame = egui::containers::Frame {
                    // #0A0A0A
                    fill: egui::Color32::from_rgba_premultiplied(10, 10, 10, 180),
                    inner_margin: egui::Margin::same(15f32),
                    outer_margin: egui::Margin::same(15f32),
                    rounding: egui::Rounding::same(8f32),
                    shadow: Shadow {
                        offset: egui::vec2(0f32, 0f32),
                        blur: 10f32,
                        spread: 5f32,
                        color: egui::Color32::from_rgba_premultiplied(0, 0, 0, 128),
                    },
                    ..Default::default()
                };

                egui::CentralPanel::default()
                .frame(my_frame)
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.label(
                            egui::widget_text::RichText::new("This is a sctk layer shell rendering an egui application with wgpu!")
                                .color(egui::Color32::WHITE),
                        );
                    });
                    ui.add(egui::widgets::Button::new("Click me!"));

                    ui.text_edit_singleline(&mut self.name);

                    /* ui.add(
                        egui::widgets::ProgressBar::new(0.5)
                            .show_percentage()
                            .animate(true)
                            .text("Progress bar to show an animation"),
                    ); */
                });
            },
        );

        // iterate over full_output.viewport_output and get the repaint_delays
        /* for (_viewport_id, output) in full_output.viewport_output.iter() {
            if !output.repaint_delay.is_zero() {
                dbg!(&output.repaint_delay);
            }
        } */

        let surface_texture = self
            .surface
            .get_current_texture()
            .expect("Failed to acquire next swap chain texture");

        let surface_view = surface_texture
            .texture
            .create_view(&egui_wgpu::wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&egui_wgpu::wgpu::CommandEncoderDescriptor { label: None });

        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [self.wgpu_config.width, self.wgpu_config.height],
            pixels_per_point: 1.0, // todo: figure out where to get that from
        };

        self.egui_state.draw(
            &self.device,
            &self.queue,
            &mut encoder,
            &surface_view,
            screen_descriptor,
            full_output,
        );

        self.layer
            .wl_surface()
            .frame(qh, self.layer.wl_surface().clone());

        self.queue.submit(Some(encoder.finish()));

        surface_texture.present();
    }
}

impl CompositorHandler for WgpuEguiLayer {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_factor: i32,
    ) {
        // Not needed for this example.
    }

    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_transform: wl_output::Transform,
    ) {
        // Not needed for this example.
    }

    fn frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
        self.can_draw = true;
    }

    fn surface_enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
        // Not needed for this example
    }

    fn surface_leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
        // Not needed for this example
    }
}

impl OutputHandler for WgpuEguiLayer {
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

impl SeatHandler for WgpuEguiLayer {
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
        if capability == Capability::Pointer && self.pointer.is_none() {
            println!("Set pointer capability");
            let pointer = self
                .seat_state
                .get_pointer(qh, &seat)
                .expect("Failed to create pointer");
            self.pointer = Some(pointer);
        }
    }

    fn remove_capability(
        &mut self,
        _conn: &Connection,
        _: &QueueHandle<Self>,
        _: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Pointer && self.pointer.is_some() {
            println!("Unset pointer capability");
            self.pointer.take().unwrap().release();
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

delegate_compositor!(WgpuEguiLayer);
delegate_output!(WgpuEguiLayer);

delegate_seat!(WgpuEguiLayer);

delegate_registry!(WgpuEguiLayer);

delegate_layer!(WgpuEguiLayer);

impl ProvidesRegistryState for WgpuEguiLayer {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState];
}

impl LayerShellHandler for WgpuEguiLayer {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {
        self.exit = true;
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _layer: &LayerSurface,
        _configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        if self.first_configure {
            self.first_configure = false;
            //self.draw(qh);
            self.can_draw = true;
        }
    }
}

delegate_pointer!(WgpuEguiLayer);

impl PointerHandler for WgpuEguiLayer {
    fn pointer_frame(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        for event in events {
            let egui_event = match event.kind {
                PointerEventKind::Enter { .. } => egui::Event::PointerMoved(egui::pos2(
                    event.position.0 as f32,
                    event.position.1 as f32,
                )),
                PointerEventKind::Leave { .. } => egui::Event::PointerGone,
                PointerEventKind::Motion { .. } => egui::Event::PointerMoved(egui::pos2(
                    event.position.0 as f32,
                    event.position.1 as f32,
                )),
                PointerEventKind::Press { button, .. } => {
                    if let Some(button) = translate_button(button) {
                        egui::Event::PointerButton {
                            button,
                            modifiers: self.egui_state.modifiers(),
                            pos: egui::pos2(event.position.0 as f32, event.position.1 as f32),
                            pressed: true,
                        }
                    } else {
                        continue;
                    }
                }
                PointerEventKind::Release { button, .. } => {
                    if let Some(button) = translate_button(button) {
                        egui::Event::PointerButton {
                            button,
                            modifiers: self.egui_state.modifiers(),
                            pos: egui::pos2(event.position.0 as f32, event.position.1 as f32),
                            pressed: false,
                        }
                    } else {
                        continue;
                    }
                }
                // todo: implement axis event
                _ => continue,
            };
            //dbg!(&egui_event);
            self.egui_state.push_event(egui_event);
            self.has_events = true;
        }
    }
}

fn translate_button(button: u32) -> Option<PointerButton> {
    match button {
        0x110 => Some(PointerButton::Primary),
        0x111 => Some(PointerButton::Secondary),
        0x112 => Some(PointerButton::Middle),
        0x113 => Some(PointerButton::Extra1),
        0x114 => Some(PointerButton::Extra2),
        _ => None,
    }
}
