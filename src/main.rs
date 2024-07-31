use std::{ffi::CString, num::NonZeroU32, ptr::NonNull, sync::Arc};

use egui::{PointerButton, RawInput, ViewportId};
use glutin::{
    api::egl::{
        context::{NotCurrentContext, PossiblyCurrentContext},
        surface::Surface,
    },
    config::ConfigSurfaceTypes,
    display::GetGlDisplay,
    prelude::{GlDisplay, NotCurrentGlContext},
    surface::{GlSurface, WindowSurface},
};
use gui::keys::handle_key_press;
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_keyboard, delegate_layer, delegate_output, delegate_pointer,
    delegate_registry, delegate_seat,
    output::{OutputHandler, OutputState},
    reexports::{
        calloop::{EventLoop, LoopHandle},
        calloop_wayland_source::WaylandSource,
    },
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers},
        pointer::{PointerEvent, PointerEventKind, PointerHandler},
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
use wayland_client::{
    globals::registry_queue_init,
    protocol::{
        wl_keyboard, wl_output, wl_pointer, wl_seat,
        wl_surface::{self},
    },
    Connection, Proxy, QueueHandle,
};

fn main() {
    env_logger::init();

    let conn = Connection::connect_to_env().unwrap();

    let (globals, event_queue) = registry_queue_init(&conn).unwrap();
    let qh = event_queue.handle();

    let mut event_loop: EventLoop<SimpleLayer> =
        EventLoop::try_new().expect("Could not create event loop.");
    let loop_handle = event_loop.handle();
    WaylandSource::new(conn.clone(), event_queue)
        .insert(loop_handle)
        .unwrap();

    let mut simple_layer = SimpleLayer {
        registry_state: RegistryState::new(&globals),
        seat_state: SeatState::new(&globals, &qh),
        output_state: OutputState::new(&globals, &qh),
        compositor_state: CompositorState::bind(&globals, &qh)
            .expect("wl_compositor is not available"),
        layer_state: LayerShell::bind(&globals, &qh).expect("layer shell is not available"),

        exit: false,
        first_configure: true,
        width: 600,
        height: 400,
        _shift: None,
        layer: None,
        keyboard: None,
        _keyboard_focus: false,
        pointer: None,

        gl: None,
        //gl_window: None,
        surface: None,
        current_context: None,
        egui_glow: None,
        name: "Simple Layer".to_string(),
        loop_handle: event_loop.handle(),
    };

    let surface = simple_layer.compositor_state.create_surface(&qh);

    let layer = simple_layer.layer_state.create_layer_surface(
        &qh,
        surface,
        Layer::Top,
        Some("simple_layer"),
        None,
    );

    layer.set_anchor(Anchor::TOP);
    layer.set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
    layer.set_size(simple_layer.width, simple_layer.height);
    layer.set_margin(10, 10, 10, 10);

    layer.commit();

    let (not_current_context, surface) =
        init_egl(layer.wl_surface(), simple_layer.width, simple_layer.height);

    let current_context = not_current_context.make_current(&surface).unwrap();

    let gl = unsafe {
        egui_glow::glow::Context::from_loader_function(|s| {
            current_context
                .display()
                .get_proc_address(CString::new(s).unwrap().as_c_str())
        })
    };

    let gl = std::sync::Arc::new(gl);
    let egui_glow = EguiGlow::new(gl.clone());

    simple_layer.gl = Some(gl);
    //simple_layer.gl_window = Some(gl_window);
    simple_layer.surface = Some(surface);
    simple_layer.current_context = Some(current_context);
    simple_layer.egui_glow = Some(egui_glow);
    simple_layer.layer = Some(layer);

    // We don't draw immediately, the configure will notify us when to first draw.

    loop {
        event_loop.dispatch(None, &mut simple_layer).unwrap();

        if simple_layer.exit {
            println!("exiting example");
            break;
        }
    }
}

struct EguiGlow {
    pub egui_ctx: egui::Context,
    pub painter: egui_glow::Painter,
    pub egui_input: RawInput,

    shapes: Vec<egui::epaint::ClippedShape>,
    textures_delta: egui::TexturesDelta,
    start_time: std::time::Instant,
}

impl EguiGlow {
    fn new(gl: std::sync::Arc<egui_glow::glow::Context>) -> Self {
        let painter = egui_glow::Painter::new(gl, "", None).expect("failed to create painter");

        let mut input = egui::RawInput {
            focused: true,
            ..Default::default()
        };

        input.viewports.entry(ViewportId::ROOT).or_default();

        Self {
            egui_ctx: Default::default(),
            egui_input: input,
            painter,
            shapes: Default::default(),
            textures_delta: Default::default(),
            start_time: std::time::Instant::now(),
        }
    }

    fn run(&mut self, _size: (u32, u32), run_ui: impl FnMut(&egui::Context)) {
        self.egui_input.time = Some(self.start_time.elapsed().as_secs_f64());
        self.egui_input.viewport_id = ViewportId::ROOT;

        let output = self.egui_ctx.run(self.egui_input.take(), run_ui);

        self.shapes = output.shapes;
        self.textures_delta.append(output.textures_delta);
    }

    fn paint(&mut self, size: (u32, u32)) {
        let shapes = std::mem::take(&mut self.shapes);
        let mut textures_delta = std::mem::take(&mut self.textures_delta);

        for (id, image_delta) in textures_delta.set {
            self.painter.set_texture(id, &image_delta);
        }

        let clipped_primitives = self
            .egui_ctx
            .tessellate(shapes, self.egui_ctx.pixels_per_point());
        let dimensions: [u32; 2] = [size.0, size.1];
        self.painter.paint_primitives(
            dimensions,
            self.egui_ctx.pixels_per_point(),
            &clipped_primitives,
        );

        for id in textures_delta.free.drain(..) {
            self.painter.free_texture(id);
        }
    }
}

struct SimpleLayer {
    registry_state: RegistryState,
    seat_state: SeatState,
    output_state: OutputState,
    compositor_state: CompositorState,
    layer_state: LayerShell,

    exit: bool,
    first_configure: bool,
    width: u32,
    height: u32,
    _shift: Option<u32>,
    layer: Option<LayerSurface>,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    _keyboard_focus: bool,
    pointer: Option<wl_pointer::WlPointer>,

    gl: Option<Arc<egui_glow::glow::Context>>,
    //gl_window: Option<glutin::RawContext<glutin::PossiblyCurrent>>,
    surface: Option<Surface<WindowSurface>>,
    current_context: Option<PossiblyCurrentContext>,
    egui_glow: Option<EguiGlow>,
    name: String,
    loop_handle: LoopHandle<'static, SimpleLayer>,
}

impl CompositorHandler for SimpleLayer {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_factor: i32,
    ) {
        // Not needed for this example.
    }

    fn frame(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
        self.draw(qh);
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

    fn surface_enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
        // send focus event on input
        let egui_glow = self.egui_glow.as_mut().unwrap();
        egui_glow.egui_input.focused = true;
        egui_glow
            .egui_input
            .events
            .push(egui::Event::WindowFocused(true));
    }

    fn surface_leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
        // send focus event on input
        let egui_glow = self.egui_glow.as_mut().unwrap();
        egui_glow.egui_input.focused = false;
        egui_glow
            .egui_input
            .events
            .push(egui::Event::WindowFocused(false));
    }
}

impl OutputHandler for SimpleLayer {
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

impl LayerShellHandler for SimpleLayer {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {
        self.exit = true;
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        _layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        if configure.new_size.0 == 0 || configure.new_size.1 == 0 {
            self.width = 600;
            self.height = 400;
        } else {
            self.width = configure.new_size.0;
            self.height = configure.new_size.1;
        }

        // TODO: resize

        // Initiate the first draw.
        if self.first_configure {
            self.first_configure = false;
            self.draw(qh);
        }
    }
}

impl SeatHandler for SimpleLayer {
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
        if capability == Capability::Keyboard && self.keyboard.is_none() {
            println!("Set keyboard capability");
            let keyboard = self
                .seat_state
                .get_keyboard_with_repeat(
                    qh,
                    &seat,
                    None,
                    self.loop_handle.clone(),
                    Box::new(|state, _wl_kbd, event| {
                        
                        handle_key_press(
                            event,
                            true,
                            &mut state.egui_glow.as_mut().unwrap().egui_input,
                        );
                    }),
                )
                .expect("Failed to create keyboard");
            self.keyboard = Some(keyboard);
        }

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
        if capability == Capability::Keyboard && self.keyboard.is_some() {
            println!("Unset keyboard capability");
            self.keyboard.take().unwrap().release();
        }

        if capability == Capability::Pointer && self.pointer.is_some() {
            println!("Unset pointer capability");
            self.pointer.take().unwrap().release();
        }
    }

    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl KeyboardHandler for SimpleLayer {
    fn enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: &wl_surface::WlSurface,
        _: u32,
        _: &[u32],
        _: &[Keysym],
    ) {
        let egui_glow = self.egui_glow.as_mut().unwrap();
        egui_glow.egui_input.focused = true;
        egui_glow
            .egui_input
            .events
            .push(egui::Event::WindowFocused(true));
    }

    fn leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: &wl_surface::WlSurface,
        _: u32,
    ) {
        let egui_glow = self.egui_glow.as_mut().unwrap();
        egui_glow.egui_input.focused = false;
        egui_glow
            .egui_input
            .events
            .push(egui::Event::WindowFocused(false));
    }

    fn press_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        handle_key_press(
            event,
            true,
            &mut self.egui_glow.as_mut().unwrap().egui_input,
        );
    }

    fn release_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        handle_key_press(
            event,
            false,
            &mut self.egui_glow.as_mut().unwrap().egui_input,
        );
    }

    fn update_modifiers(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _serial: u32,
        modifiers: Modifiers,
        _layout: u32,
    ) {
        let egui_glow = self.egui_glow.as_mut().unwrap();
        egui_glow.egui_input.modifiers = egui::Modifiers {
            alt: modifiers.alt,
            ctrl: modifiers.ctrl,
            shift: modifiers.shift,
            mac_cmd: false,
            command: modifiers.ctrl,
        };
    }

    fn update_repeat_info(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _info: smithay_client_toolkit::seat::keyboard::RepeatInfo,
    ) {
        dbg!(_info);
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

impl PointerHandler for SimpleLayer {
    fn pointer_frame(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        use PointerEventKind::*;

        let egui_glow = self.egui_glow.as_mut().unwrap();

        for event in events {
            let egui_event = match event.kind {
                Press { button, .. } => {
                    if let Some(button) = translate_button(button) {
                        egui::Event::PointerButton {
                            button,
                            modifiers: egui_glow.egui_input.modifiers,
                            pos: egui::pos2(event.position.0 as f32, event.position.1 as f32),
                            pressed: true,
                        }
                    } else {
                        continue;
                    }
                }
                Release { button, .. } => {
                    println!("Release {:x} @ {:?}", button, event.position);
                    if let Some(button) = translate_button(button) {
                        egui::Event::PointerButton {
                            button,
                            modifiers: egui_glow.egui_input.modifiers,
                            pos: egui::pos2(event.position.0 as f32, event.position.1 as f32),
                            pressed: false,
                        }
                    } else {
                        continue;
                    }
                }
                Enter { .. } => egui::Event::PointerMoved(egui::pos2(
                    event.position.0 as f32,
                    event.position.1 as f32,
                )),
                Motion { .. } => egui::Event::PointerMoved(egui::pos2(
                    event.position.0 as f32,
                    event.position.1 as f32,
                )),
                Leave { .. } => egui::Event::PointerGone,
                _ => {
                    continue;
                }
            };

            egui_glow.egui_input.events.push(egui_event);
        }
    }
}

impl SimpleLayer {
    pub fn draw(&mut self, qh: &QueueHandle<Self>) {
        let window = self.layer.as_ref().unwrap();
        let gl = self.gl.as_mut().unwrap();
        let surface = self.surface.as_mut().unwrap();
        let context = self.current_context.as_mut().unwrap();

        let egui_glow = self.egui_glow.as_mut().unwrap();

        let _repaint_after = egui_glow.run((self.width, self.height), |egui_ctx| {
            egui::CentralPanel::default().show(egui_ctx, |ui| {
                ui.horizontal(|ui| {
                    let name_label = ui.label("Your name: ");
                    ui.text_edit_singleline(&mut self.name)
                        .labelled_by(name_label.id);
                });
                if ui.button("click me").clicked() {
                    println!("Button clicked!");
                }
                ui.label(format!("Hello '{}'", self.name));
            });
        });

        egui_glow::painter::clear(
            gl,
            [self.width, self.height],
            egui::Rgba::from_rgba_unmultiplied(0f32, 0f32, 0f32, 0f32).to_array(),
        );

        egui_glow.paint((self.width, self.height));

        window.wl_surface().frame(qh, window.wl_surface().clone());

        surface
            .swap_buffers(context)
            .expect("failed to swap buffers");
    }
}

delegate_compositor!(SimpleLayer);
delegate_output!(SimpleLayer);

delegate_seat!(SimpleLayer);
delegate_keyboard!(SimpleLayer);
delegate_pointer!(SimpleLayer);

delegate_layer!(SimpleLayer);

delegate_registry!(SimpleLayer);

impl ProvidesRegistryState for SimpleLayer {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState, SeatState];
}

fn init_egl(
    surface: &wl_surface::WlSurface,
    width: u32,
    height: u32,
) -> (NotCurrentContext, Surface<glutin::surface::WindowSurface>) {
    let display_pointer = surface
        .backend()
        .upgrade()
        .expect("Connection has been closed")
        .display_ptr() as *mut _;

    let display_handle =
        raw_window_handle::WaylandDisplayHandle::new(NonNull::new(display_pointer).unwrap());

    let display_handle = raw_window_handle::RawDisplayHandle::Wayland(display_handle);
    let window_handle = raw_window_handle::WaylandWindowHandle::new(
        NonNull::new(surface.id().as_ptr() as *mut _).unwrap(),
    );
    let window_handle = raw_window_handle::RawWindowHandle::Wayland(window_handle);

    let display = unsafe { glutin::api::egl::display::Display::new(display_handle) }
        .expect("Failed to initialize Wayland EGL platform");

    // Find a suitable config for the window.
    let config_template = glutin::config::ConfigTemplateBuilder::default()
        .compatible_with_native_window(window_handle)
        .with_surface_type(ConfigSurfaceTypes::WINDOW)
        .with_api(
            glutin::config::Api::GLES2 | glutin::config::Api::GLES3 | glutin::config::Api::OPENGL,
        )
        .build();
    let display_config = unsafe { display.find_configs(config_template) }
        .unwrap()
        .next()
        .expect("No available configs");
    let gl_attributes = glutin::context::ContextAttributesBuilder::default()
        .with_context_api(glutin::context::ContextApi::OpenGl(None))
        .build(Some(window_handle));
    let gles_attributes = glutin::context::ContextAttributesBuilder::default()
        .with_context_api(glutin::context::ContextApi::Gles(None))
        .build(Some(window_handle));

    // Create a context, trying OpenGL and then Gles.
    let not_current_context = unsafe { display.create_context(&display_config, &gl_attributes) }
        .or_else(|_| unsafe { display.create_context(&display_config, &gles_attributes) })
        .expect("Failed to create context");

    let surface_attributes = glutin::surface::SurfaceAttributesBuilder::<WindowSurface>::default()
        .build(
            window_handle,
            NonZeroU32::new(width).unwrap(),
            NonZeroU32::new(height).unwrap(),
        );
    let surface = unsafe { display.create_window_surface(&display_config, &surface_attributes) }
        .expect("Failed to create surface");

    (not_current_context, surface)
}
