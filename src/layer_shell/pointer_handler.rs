use egui::{PointerButton, Vec2};
use smithay_client_toolkit::{
    delegate_pointer,
    seat::pointer::{PointerEvent, PointerEventKind, PointerHandler},
};
use wayland_client::{
    protocol::wl_pointer::{self},
    Connection, QueueHandle,
};

use super::WgpuLayerShellState;

delegate_pointer!(WgpuLayerShellState);

impl PointerHandler for WgpuLayerShellState {
    fn pointer_frame(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        for event in events {
            let position = egui::pos2(event.position.0 as f32, event.position.1 as f32);
            let egui_event = match event.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    egui::Event::PointerMoved(position)
                }
                PointerEventKind::Leave { .. } => egui::Event::PointerGone,
                PointerEventKind::Press { button, .. }
                | PointerEventKind::Release { button, .. } => {
                    if let Some(button) = translate_button(button) {
                        egui::Event::PointerButton {
                            button,
                            modifiers: self.egui_state.modifiers(),
                            pos: position,
                            pressed: matches!(event.kind, PointerEventKind::Press { .. }),
                        }
                    } else {
                        continue;
                    }
                }
                PointerEventKind::Axis {
                    horizontal,
                    vertical,
                    ..
                } => egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: Vec2::new(-horizontal.absolute as f32, -vertical.absolute as f32),
                    modifiers: self.egui_state.modifiers(),
                },
            };
            self.egui_state.push_event(egui_event);
        }
    }
}

fn translate_button(button: u32) -> Option<PointerButton> {
    match button {
        0x110 => Some(PointerButton::Primary),
        0x111 => Some(PointerButton::Secondary),
        0x112 => Some(PointerButton::Middle),
        0x113 => Some(PointerButton::Extra1),
        0x114 => Some(PointerButton::Extra2),
        _ => None,
    }
}
