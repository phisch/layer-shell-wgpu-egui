use layer_shell_wgpu_egui::layer_shell::LayerShellOptions;

fn main() -> layer_shell_wgpu_egui::Result {
    env_logger::init();

    let options = LayerShellOptions {
        ..Default::default()
    };

    // application state
    let mut name = "Arthur".to_owned();
    let mut age = 42;

    layer_shell_wgpu_egui::run_layer_simple(options, move |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("My egui Layer");
            ui.horizontal(|ui| {
                let name_label = ui.label("Your name: ");
                ui.text_edit_singleline(&mut name)
                    .labelled_by(name_label.id);
            });
            ui.add(egui::Slider::new(&mut age, 0..=120).text("age"));
            if ui.button("Increment").clicked() {
                age += 1;
            }
            ui.label(format!("Hello '{name}', age {age}"));
        });
    })?;

    Ok(())
}
