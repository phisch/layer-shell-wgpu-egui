use egui::{scroll_area::ScrollBarVisibility, Vec2};
use layer_shell_wgpu_egui::layer_shell::LayerShellOptions;

fn main() -> layer_shell_wgpu_egui::Result {
    env_logger::init();

    let options = LayerShellOptions {
        height: 600,
        width: 600,
        ..Default::default()
    };

    layer_shell_wgpu_egui::run_layer(
        options,
        Box::new(|egui_context| {
            egui_extras::install_image_loaders(&egui_context);
            Ok(Box::<MyApp>::default())
        }),
    )
}

#[derive(Default)]
struct MyApp {}

impl layer_shell_wgpu_egui::App for MyApp {
    fn update(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .hscroll(true)
                .scroll_bar_visibility(ScrollBarVisibility::AlwaysHidden)
                .show(ui, |ui| {
                    ui.add(
                        egui::Image::new(egui::include_image!("assets/ferris.svg"))
                            .rounding(10.0)
                            .fit_to_fraction(Vec2::new(3.0, 3.0)),
                    );
                });
        });
    }
}
