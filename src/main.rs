use std::{ffi::CString, num::NonZeroU32, ptr::NonNull, sync::Arc};

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
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_keyboard, delegate_layer, delegate_output, delegate_pointer,
    delegate_registry, delegate_seat,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers},
        pointer::{PointerEvent, PointerEventKind, PointerHandler},
        Capability, SeatHandler, SeatState,
    },
    shell::{
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface, LayerSurfaceConfigure
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

    let (globals, mut event_queue) = registry_queue_init(&conn).unwrap();
    let qh = event_queue.handle();

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
        shift: None,
        layer: None,
        keyboard: None,
        keyboard_focus: false,
        pointer: None,

        gl: None,
        //gl_window: None,
        surface: None,
        current_context: None,
        egui_glow: None,
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
        event_queue.blocking_dispatch(&mut simple_layer).unwrap();

        if simple_layer.exit {
            println!("exiting example");
            break;
        }
    }
}

struct EguiGlow {
    pub egui_ctx: egui::Context,
    pub painter: egui_glow::Painter,

    shapes: Vec<egui::epaint::ClippedShape>,
    textures_delta: egui::TexturesDelta,
}

impl EguiGlow {
    fn new(gl: std::sync::Arc<egui_glow::glow::Context>) -> Self {
        let painter = egui_glow::Painter::new(gl, "", None).expect("failed to create painter");

        Self {
            egui_ctx: Default::default(),
            painter,
            shapes: Default::default(),
            textures_delta: Default::default(),
        }
    }

    fn run(&mut self, size: (u32, u32), run_ui: impl FnMut(&egui::Context)) {
        let egui::FullOutput {
            platform_output: _platform_output,
            textures_delta,
            shapes,
            pixels_per_point: _pixels_per_point,
            viewport_output: _viewport_output,
        } = self.egui_ctx.run(
            egui::RawInput {
                screen_rect: Some({
                    egui::Rect {
                        min: egui::Pos2 { x: 0f32, y: 0f32 },
                        max: egui::Pos2 {
                            x: size.0 as f32,
                            y: size.1 as f32,
                        },
                    }
                }),
                ..Default::default()
            },
            run_ui,
        );
        self.shapes = shapes;
        self.textures_delta.append(textures_delta);
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
    shift: Option<u32>,
    layer: Option<LayerSurface>,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    keyboard_focus: bool,
    pointer: Option<wl_pointer::WlPointer>,

    gl: Option<Arc<egui_glow::glow::Context>>,
    //gl_window: Option<glutin::RawContext<glutin::PossiblyCurrent>>,
    surface: Option<Surface<WindowSurface>>,
    current_context: Option<PossiblyCurrentContext>,
    egui_glow: Option<EguiGlow>,
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
        // not needed
    }

    fn surface_leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
        // not needed
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
                .get_keyboard(qh, &seat, None)
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
        surface: &wl_surface::WlSurface,
        _: u32,
        _: &[u32],
        keysyms: &[Keysym],
    ) {
        if self.layer.as_ref().map(LayerSurface::wl_surface) == Some(surface) {
            println!("Keyboard focus on window with pressed syms: {:?}", keysyms);
            self.keyboard_focus = true;
        }
    }

    fn leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        surface: &wl_surface::WlSurface,
        _: u32,
    ) {
        if self.layer.as_ref().map(LayerSurface::wl_surface) == Some(surface) {
            println!("Release keyboard focus on window");
            self.keyboard_focus = false;
        }
    }

    fn press_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        println!("Key press: {:?}", event);
    }

    fn release_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_keyboard::WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        println!("Key release: {:?}", event);
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
        println!("Update modifiers: {:?}", modifiers);
    }
}

impl PointerHandler for SimpleLayer {
    fn pointer_frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _pointer: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        use PointerEventKind::*;

        let egui_glow = self.egui_glow.as_mut().unwrap();

        let egui_context = &mut egui_glow.egui_ctx;

        for event in events {
            if Some(&event.surface) != self.layer.as_ref().map(LayerSurface::wl_surface) {
                continue;
            }
            match event.kind {
                Enter { .. } => {
                    println!("Pointer entered @{:?}", event.position);
                }
                Leave { .. } => {
                    println!("Pointer left");
                }
                Motion { .. } => {
                    println!("Pointer moved to {:?}", event.position);
                    egui_context.input_mut(|input_state| {
                        input_state
                            .events
                            .push(egui::Event::PointerMoved(egui::pos2(
                                event.position.0 as f32,
                                event.position.1 as f32,
                            )))
                    });
                }
                Press { button, .. } => {
                    println!("Press {:x} @ {:?}", button, event.position);
                    egui_context.input_mut(|input_state| {
                        input_state.events.push(egui::Event::PointerButton {
                            button: egui::PointerButton::Primary,
                            modifiers: Default::default(),
                            pos: egui::pos2(event.position.0 as f32, event.position.1 as f32),
                            pressed: true,
                        })
                    });
                    self.shift = self.shift.xor(Some(0));
                }
                Release { button, .. } => {
                    println!("Release {:x} @ {:?}", button, event.position);
                }
                Axis {
                    horizontal,
                    vertical,
                    ..
                } => {
                    println!("Scroll H:{:?}, V:{:?}", horizontal, vertical);
                }
            }
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
            let my_frame = egui::containers::Frame {
                fill: egui::Color32::DARK_GRAY,
                inner_margin: egui::Margin {
                    left: 10f32,
                    right: 10f32,
                    top: 10f32,
                    bottom: 10f32,
                },
                rounding: egui::Rounding::same(10f32),
                ..Default::default()
            };

            egui::CentralPanel::default()
                .frame(my_frame)
                .show(egui_ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.label(
                            egui::widget_text::RichText::new("Main text")
                                .color(egui::Color32::WHITE),
                        );
                    });
                    ui.add(
                        egui::widgets::ProgressBar::new(0.5)
                            .show_percentage()
                            .animate(true)
                            .text("Text here?"),
                    );
                    ui.add(egui::widgets::Button::new("Click me"));
                    ui.columns(3, |columns| {
                        columns[0].label(
                            egui::widget_text::RichText::new("Some status")
                                .color(egui::Color32::WHITE),
                        );
                        columns[2].with_layout(
                            egui::Layout::right_to_left(egui::Align::Min),
                            |ui| {
                                ui.label(
                                    egui::widget_text::RichText::new("0%")
                                        .color(egui::Color32::WHITE),
                                );
                            },
                        );
                    });
                    egui::warn_if_debug_build(ui);
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
