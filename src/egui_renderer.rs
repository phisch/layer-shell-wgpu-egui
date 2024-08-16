use egui::{Context, ViewportId};
use egui_wgpu::{Renderer, ScreenDescriptor};
use egui_wgpu::wgpu::{Color, CommandEncoder, Device, LoadOp, Operations, Queue, RenderPassColorAttachment, RenderPassDescriptor, StoreOp, TextureFormat, TextureView};

use crate::egui_state::State;

pub struct EguiRenderer {
    state: State,
    renderer: Renderer,
}

impl EguiRenderer {

    pub fn new(
        device: &Device,
        output_color_format: TextureFormat,
        output_depth_format: Option<TextureFormat>,
        msaa_samples: u32
    ) -> EguiRenderer {
        let egui_context = Context::default();

        let egui_state = State::new(
            egui_context,
            egui::viewport::ViewportId::ROOT,
        );
        let egui_renderer = Renderer::new(
            device,
            output_color_format,
            output_depth_format,
            msaa_samples,
        );

        EguiRenderer {
            state: egui_state,
            renderer: egui_renderer,
        }
    }

    /* pub fn handle_input(&mut self, window: &Window, event: &WindowEvent) {
        self.state.on_window_event(window, event);
    } */

    /* pub fn ppp(&mut self, v: f32) {
        self.state.egui_ctx().set_pixels_per_point(v);
    } */

    pub fn draw(
        &mut self,
        device: &Device,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        window_surface_view: &TextureView,
        screen_descriptor: ScreenDescriptor,
        run_ui: impl FnOnce(&Context),
    ) {
        self.state
            .context()
            .set_pixels_per_point(screen_descriptor.pixels_per_point);


        // TODO: maybe we need to take input for a certain window / surface?
        let raw_input = self.state.take_input();
        if (&raw_input.events).len() > 0 {
            dbg!(&raw_input.events);
        }
        let full_output = self.state.context().run(raw_input, |ui| {
            run_ui(ui);
        });


        // iterate over viewport outputs
        /* for output in full_output.viewport_output.values() {
            dbg!(&output.repaint_delay);
        } */

        //dbg!(&full_output.);

        // TODO: implement platform output handling
        // this is for things like clipboard support
        //self.state.handle_platform_output(window, full_output.platform_output);

        let tris = self
            .state
            .context()
            .tessellate(full_output.shapes, self.state.context().pixels_per_point());
        for (id, image_delta) in &full_output.textures_delta.set {
            self.renderer
                .update_texture(device, queue, *id, image_delta);
        }
        self.renderer
            .update_buffers(device, queue, encoder, &tris, &screen_descriptor);
        let mut rpass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("egui main render pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: window_surface_view,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(egui_wgpu::wgpu::Color { // Explicitly define the clear color with transparency
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 0.0,
                    }),
                    store: StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        self.renderer.render(&mut rpass, &tris, &screen_descriptor);
        drop(rpass);
        for x in &full_output.textures_delta.free {
            self.renderer.free_texture(x)
        }
    }
}