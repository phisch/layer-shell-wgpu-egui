#[derive(Debug)]
pub enum Error {
    Wgpu(egui_wgpu::WgpuError),
}