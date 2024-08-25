use layer_shell_wgpu_egui::layer_shell::LayerShellOptions;
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity};

fn main() -> layer_shell_wgpu_egui::Result {
    env_logger::init();

    let options = LayerShellOptions {
        height: 100,
        anchor: Some(Anchor::TOP | Anchor::LEFT | Anchor::RIGHT),
        keyboard_interactivity: Some(KeyboardInteractivity::OnDemand),
        ..Default::default()
    };

    layer_shell_wgpu_egui::run_layer(
        options,
        Box::new(|egui_context| {
            // This gives us image support:
            egui_extras::install_image_loaders(&egui_context);
            Ok(Box::<MyApp>::default())
        }),
    )
}

#[derive(Default)]
struct MyApp {
    name: String,
    age: u8,
}

impl layer_shell_wgpu_egui::App for MyApp {
    fn update(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                let name_label = ui.label("Your name: ");
                ui.text_edit_singleline(&mut self.name)
                    .labelled_by(name_label.id);
            });
            if ui.button("click me").clicked() {
                println!("Button clicked!");
            }
            ui.label(format!("Hello '{}', age {}", self.name, self.age));
        });
    }
}
