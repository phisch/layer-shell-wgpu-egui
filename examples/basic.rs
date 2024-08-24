fn main() -> layer_shell_wgpu_egui::Result {
    env_logger::init();

    let options = layer_shell_wgpu_egui::LayerShellOptions {
        ..Default::default()
    };

    //egui_sctk::run_layer_shell(options, Ok(Box::<MyApp>::default()))
    Ok(())
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
