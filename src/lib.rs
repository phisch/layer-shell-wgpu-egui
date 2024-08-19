pub mod error;
pub mod wgpu_state;

use wayland_client::Connection;

//pub mod egui_renderer;
pub mod egui_state;

pub mod keys;


#[derive(Debug)]
pub enum Error {
    AppCreation(Box<dyn std::error::Error + Send + Sync>),
    Wgpu(egui_wgpu::WgpuError),
}

/// Short for `Result<T, eframe::Error>`.
pub type Result<T = (), E = Error> = std::result::Result<T, E>;

#[derive(Default)]
pub struct LayerShellOptions {
    pub something: u32,
}

pub trait App {

    fn update(&mut self, ctx: &egui::Context);

    // fn save(&mut self, _storage: &mut dyn Storage) {}
    // fn on_exit(&mut self) {}
    // fn auto_save_interval(&self) -> std::time::Duration {
    //     std::time::Duration::from_secs(30)
    // }
    // fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
    //     egui::Color32::from_rgba_unmultiplied(12, 12, 12, 180).to_normalized_gamma_f32()
    // }
}

struct WgpuLayerShellApp {
    layer_shell_options: LayerShellOptions,
    app: Box<dyn App>,
}

impl WgpuLayerShellApp {
    pub fn new(layer_shell_options: LayerShellOptions, app: impl App + 'static) -> Self {

        //let connection = Connection::connect_to_env();







        Self {
            layer_shell_options,
            app: Box::new(app),
        }
    }

    pub fn run(self) -> Result {
        Ok(())
    }
}


pub fn run_layer_shell(
    options: LayerShellOptions,
    app: impl App + 'static,
) -> Result {
    let app = WgpuLayerShellApp::new(options, app);
    app.run()
}
