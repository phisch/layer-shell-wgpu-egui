use application::WgpuLayerShellApp;
use layer_shell::LayerShellOptions;

pub(crate) mod application;
pub(crate) mod egui_state;
pub mod error;
pub mod layer_shell;
pub(crate) mod wgpu_state;

#[derive(Debug)]
pub enum Error {
    AppCreation(Box<dyn std::error::Error + Send + Sync>),
    Wgpu(egui_wgpu::WgpuError),
}

/// Short for `Result<T, eframe::Error>`.
pub type Result<T = (), E = Error> = std::result::Result<T, E>;

pub type AppCreator = Box<dyn FnOnce(&egui::Context) -> Result<Box<dyn App>, Error>>;

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

pub fn run_layer(options: LayerShellOptions, app_creator: AppCreator) -> Result {
    let mut app = WgpuLayerShellApp::new(options, app_creator);

    app.run()
}

pub fn run_layer_simple(
    options: LayerShellOptions,
    update_fun: impl FnMut(&egui::Context) + 'static,
) -> Result {
    struct SimpleLayerWrapper<U> {
        update_fun: U,
    }

    impl<U: FnMut(&egui::Context) + 'static> App for SimpleLayerWrapper<U> {
        fn update(&mut self, ctx: &egui::Context) {
            (self.update_fun)(ctx);
        }
    }

    run_layer(
        options,
        Box::new(|_| Ok(Box::new(SimpleLayerWrapper { update_fun }))),
    )
}
