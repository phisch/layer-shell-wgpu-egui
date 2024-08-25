use std::time::Instant;

use smithay_client_toolkit::reexports::calloop::EventLoop;

use crate::{
    layer_shell::{LayerShellOptions, WgpuLayerShellState},
    AppCreator, Result,
};

pub struct WgpuLayerShellApp {
    wgpu_layer_shell_state: WgpuLayerShellState,
    event_loop: EventLoop<'static, WgpuLayerShellState>,
}

impl WgpuLayerShellApp {
    pub fn new(_layer_shell_options: LayerShellOptions, _app_creator: AppCreator) -> Self {
        let event_loop: EventLoop<WgpuLayerShellState> =
            EventLoop::try_new().expect("Could not create event loop.");
        let wgpu = WgpuLayerShellState::new(&event_loop);
        Self {
            wgpu_layer_shell_state: wgpu,
            event_loop,
        }
    }

    pub fn run(&mut self) -> Result {
        loop {
            let timeout = match *self.wgpu_layer_shell_state.redraw_at.read().unwrap() {
                Some(instant) => {
                    if self.wgpu_layer_shell_state.can_draw {
                        Some(instant.duration_since(Instant::now()))
                    } else {
                        None
                    }
                }
                None => None,
            };

            self.event_loop
                .dispatch(timeout, &mut self.wgpu_layer_shell_state)
                .unwrap();

            if self.wgpu_layer_shell_state.should_draw() {
                self.wgpu_layer_shell_state.draw();
            }

            if self.wgpu_layer_shell_state.exit {
                println!("exiting example");
                break;
            }
        }
        Ok(())
    }
}
