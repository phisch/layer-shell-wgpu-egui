pub mod keys;

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
