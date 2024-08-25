use std::{
    cell::RefCell,
    time::{Duration, Instant},
};

use smithay_client_toolkit::reexports::calloop::EventLoop;

use crate::{
    layer_shell::{LayerShellOptions, WgpuLayerShellState},
    App, AppCreator, Result,
};

pub struct WgpuLayerShellApp {
    application: RefCell<Box<dyn App>>,
    event_loop: EventLoop<'static, WgpuLayerShellState>,
    layer_shell_state: WgpuLayerShellState,
}

impl WgpuLayerShellApp {
    pub fn new(layer_shell_options: LayerShellOptions, app_creator: AppCreator) -> Self {
        let event_loop: EventLoop<WgpuLayerShellState> =
            EventLoop::try_new().expect("Could not create event loop.");
        let layer_shell_state = WgpuLayerShellState::new(&event_loop, layer_shell_options);

        Self {
            // TODO: find better way to handle this potential error
            application: RefCell::new(
                app_creator(&layer_shell_state.egui_state.context()).expect("could not create app"),
            ),
            event_loop,
            layer_shell_state,
        }
    }

    pub fn run(&mut self) -> Result {
        loop {
            self.event_loop
                .dispatch(
                    self.layer_shell_state.get_timeout(),
                    &mut self.layer_shell_state,
                )
                .unwrap();

            if self.layer_shell_state.should_draw() {
                let mut application = self.application.borrow_mut();
                self.layer_shell_state.draw(&mut **application);
            }

            if self.layer_shell_state.exit {
                println!("exiting example");
                break;
            }
        }
        Ok(())
    }
}
