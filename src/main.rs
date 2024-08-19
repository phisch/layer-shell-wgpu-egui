use egui::{PointerButton, Shadow};
use egui_sctk::wgpu_state::WgpuState;
use egui_wgpu::ScreenDescriptor;
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_layer, delegate_output, delegate_pointer, delegate_registry,
    delegate_seat,
    output::{OutputHandler, OutputState},
    reexports::{
        calloop::EventLoop,
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
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use wayland_client::{
    globals::registry_queue_init,
    protocol::{wl_output, wl_pointer, wl_seat, wl_surface},
    Connection, QueueHandle,
};

fn main() {
    env_logger::init();

    let initial_width = 600;
    let initial_height = 300;

    let connection = Connection::connect_to_env().unwrap();
    let (global_list, event_queue) = registry_queue_init(&connection).unwrap();
    let queue_handle: Arc<QueueHandle<WgpuEguiLayer>> = Arc::new(event_queue.handle());

    let mut event_loop: EventLoop<WgpuEguiLayer> =
        EventLoop::try_new().expect("Could not create event loop.");

    WaylandSource::new(connection.clone(), event_queue)
        .insert(event_loop.handle())
        .unwrap();

    let compositor_state =
        CompositorState::bind(&global_list, &queue_handle).expect("wl_compositor not available");

    let wl_surface = compositor_state.create_surface(&queue_handle);

    let layer_shell =
        LayerShell::bind(&global_list, &queue_handle).expect("layer shell not available");
    let layer_surface = layer_shell.create_layer_surface(
        &queue_handle,
        wl_surface,
        Layer::Top,
        Some("wgpu_egui_layer"),
        None,
    );
    layer_surface.set_anchor(Anchor::TOP);
    layer_surface.set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
    layer_surface.set_size(initial_width, initial_height);
    layer_surface.commit();

    let wgpu_state = WgpuState::new(&connection.backend(), layer_surface.wl_surface()).unwrap();

    let egui_context = egui::Context::default();

    let redraw_at = Arc::new(Mutex::new(None));

    egui_context.set_request_repaint_callback({
        let redraw_at = Arc::clone(&redraw_at);
        move |info| {
            let mut redraw_at = redraw_at.lock().unwrap();
            if info.delay == Duration::ZERO {
                *redraw_at = Some(Instant::now());
            } else {
                *redraw_at = Some(Instant::now() + info.delay);
            }

            dbg!("SET TO SOME");
        }
    });

    let egui_state = egui_sctk::egui_state::State::new(
        egui_context,
        egui::viewport::ViewportId::ROOT,
        &wgpu_state.device,
        wgpu_state.surface_configuration.format,
        None,
        1,
    );

    let mut wgpu = WgpuEguiLayer {
        registry_state: RegistryState::new(&global_list),
        seat_state: SeatState::new(&global_list, &queue_handle),
        output_state: OutputState::new(&global_list, &queue_handle),

        exit: false,
        width: initial_width,
        height: initial_height,
        layer: layer_surface,

        egui_state,

        first_configure: true,

        pointer: None,
        name: "foo".to_string(),

        can_draw: false,
        has_events: true,

        redraw_at: redraw_at,
        //render_state: None

        wgpu_state,
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
            wgpu.draw(&queue_handle);
        }

        if wgpu.exit {
            println!("exiting example");
            break;
        }
    }

    // On exit we must destroy the surface before the window is destroyed.
    drop(wgpu.wgpu_state.surface);
    drop(wgpu.layer);
}


struct WgpuEguiLayer {
    registry_state: RegistryState, // SCTK
    seat_state: SeatState,         // SCTK
    output_state: OutputState,     // SCTK

    exit: bool,          // Application
    width: u32,          // Application
    height: u32,         // Application
    layer: LayerSurface, // SCTK

    egui_state: egui_sctk::egui_state::State,
    //egui_renderer: EguiRenderer,

    first_configure: bool, // SCTK

    pointer: Option<wl_pointer::WlPointer>, // SCTK

    name: String, // Application

    can_draw: bool,   // SCTK should be named has_frame_callback
    has_events: bool, // SCTK

    redraw_at: Arc<Mutex<Option<Instant>>>, // SCTK but is managed by egui_state

    //render_state: egui_wgpu::RenderState

    wgpu_state: WgpuState,
}

impl WgpuEguiLayer {
    pub fn should_draw(&self) -> bool {
        let mut redraw_at = self.redraw_at.lock().unwrap();

        if !self.can_draw {
            return false;
        }

        if self.has_events {
            return true;
        }

        match redraw_at.as_ref() {
            Some(instant) => {
                if Instant::now() >= *instant {
                    *redraw_at = None;
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
            .wgpu_state.surface
            .get_current_texture()
            .expect("Failed to acquire next swap chain texture");

        let surface_view = surface_texture
            .texture
            .create_view(&egui_wgpu::wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .wgpu_state.device
            .create_command_encoder(&egui_wgpu::wgpu::CommandEncoderDescriptor { label: None });

        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [self.wgpu_state.surface_configuration.width, self.wgpu_state.surface_configuration.height],
            pixels_per_point: 1.0, // todo: figure out where to get that from
        };

        self.egui_state.draw(
            &self.wgpu_state.device,
            &self.wgpu_state.queue,
            &mut encoder,
            &surface_view,
            screen_descriptor,
            full_output,
        );

        self.layer
            .wl_surface()
            .frame(qh, self.layer.wl_surface().clone());

        self.wgpu_state.queue.submit(Some(encoder.finish()));

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
