use layer_shell_wgpu_egui::layer_shell::LayerShellOptions;
use smithay_client_toolkit::shell::wlr_layer::KeyboardInteractivity;

fn main() -> layer_shell_wgpu_egui::Result {
    env_logger::init();

    let options = LayerShellOptions {
        width: 500,
        height: 300,
        keyboard_interactivity: Some(KeyboardInteractivity::OnDemand),
        ..Default::default()
    };

    // application state
    let mut name = "Alice".to_owned();
    let mut age = 26;

    layer_shell_wgpu_egui::run_layer_simple(options, move |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum.");
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
