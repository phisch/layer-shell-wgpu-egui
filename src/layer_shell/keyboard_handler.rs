use egui::{Modifiers, RawInput};
use smithay_client_toolkit::{
    delegate_keyboard,
    seat::keyboard::{KeyEvent, KeyboardHandler, Keysym},
};
use wayland_client::{protocol::wl_surface, Connection, QueueHandle};

use super::WgpuLayerShellState;

delegate_keyboard!(WgpuLayerShellState);

impl KeyboardHandler for WgpuLayerShellState {
    fn enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wayland_client::protocol::wl_keyboard::WlKeyboard,
        _surface: &wl_surface::WlSurface,
        _serial: u32,
        _raw: &[u32],
        _keysyms: &[smithay_client_toolkit::seat::keyboard::Keysym],
    ) {
        let input = self.egui_state.input();
        input.focused = true;
        // todo: this should probably be in surface enter?
        input.events.push(egui::Event::WindowFocused(true));
    }

    fn leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wayland_client::protocol::wl_keyboard::WlKeyboard,
        _surface: &wl_surface::WlSurface,
        _serial: u32,
    ) {
        let input = self.egui_state.input();
        input.focused = false;
        // todo: this should probably be in surface enter?
        input.events.push(egui::Event::WindowFocused(false));
    }

    fn press_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wayland_client::protocol::wl_keyboard::WlKeyboard,
        _serial: u32,
        event: smithay_client_toolkit::seat::keyboard::KeyEvent,
    ) {
        handle_key_press(event, true, self.egui_state.input());
    }

    fn release_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wayland_client::protocol::wl_keyboard::WlKeyboard,
        _serial: u32,
        event: smithay_client_toolkit::seat::keyboard::KeyEvent,
    ) {
        handle_key_press(event, false, self.egui_state.input());
    }

    fn update_modifiers(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wayland_client::protocol::wl_keyboard::WlKeyboard,
        _serial: u32,
        modifiers: smithay_client_toolkit::seat::keyboard::Modifiers,
        _layout: u32,
    ) {
        self.egui_state.input().modifiers = Modifiers {
            alt: modifiers.alt,
            ctrl: modifiers.ctrl,
            shift: modifiers.shift,
            mac_cmd: false, // this is linux only
            command: modifiers.ctrl,
        };
    }
}

fn handle_clipboard_shortcuts(
    key: egui::Key,
    modifiers: Modifiers,
    egui_input: &mut RawInput,
) -> bool {
    let event = match (key, modifiers.ctrl) {
        (egui::Key::C, true) => Some(egui::Event::Copy),
        (egui::Key::X, true) => Some(egui::Event::Cut),
        (egui::Key::V, true) => {
            // todo: implement paste
            None
        }
        _ => None,
    };

    if let Some(event) = event {
        egui_input.events.push(event);
        return true;
    }

    false
}

pub fn handle_key_press(event: KeyEvent, pressed: bool, egui_input: &mut RawInput) {
    if let Some(key) = keysym_to_egui_key(event.keysym) {
        if pressed && handle_clipboard_shortcuts(key, egui_input.modifiers, egui_input) {
            return;
        }

        let key_event = egui::Event::Key {
            physical_key: None,
            repeat: false, // seems to be just handled by egui
            key,
            pressed,
            modifiers: egui_input.modifiers,
        };

        egui_input.events.push(key_event);
    }

    if let Some(utf8_string) = event.utf8 {
        if utf8_string.chars().all(is_printable_char) {
            egui_input.events.push(egui::Event::Text(utf8_string));
        }
    }
}

fn keysym_to_egui_key(keysym: Keysym) -> Option<egui::Key> {
    match keysym {
        Keysym::Down => Some(egui::Key::ArrowDown),
        Keysym::Left => Some(egui::Key::ArrowLeft),
        Keysym::Right => Some(egui::Key::ArrowRight),
        Keysym::Up => Some(egui::Key::ArrowUp),

        Keysym::Escape => Some(egui::Key::Escape),
        Keysym::Tab => Some(egui::Key::Tab),
        Keysym::BackSpace => Some(egui::Key::Backspace),
        Keysym::Return => Some(egui::Key::Enter),
        Keysym::space => Some(egui::Key::Space),

        Keysym::Insert => Some(egui::Key::Insert),
        Keysym::Delete => Some(egui::Key::Delete),
        Keysym::Home => Some(egui::Key::Home),
        Keysym::End => Some(egui::Key::End),
        Keysym::Page_Up => Some(egui::Key::PageUp),
        Keysym::Page_Down => Some(egui::Key::PageDown),

        Keysym::XF86_Copy => Some(egui::Key::Copy),
        Keysym::XF86_Cut => Some(egui::Key::Cut),
        Keysym::XF86_Paste => Some(egui::Key::Paste),

        Keysym::colon => Some(egui::Key::Colon),
        Keysym::comma => Some(egui::Key::Comma),
        Keysym::backslash => Some(egui::Key::Backslash),
        Keysym::slash => Some(egui::Key::Slash),
        Keysym::bar => Some(egui::Key::Pipe),

        Keysym::question => Some(egui::Key::Questionmark),
        Keysym::parenleft => Some(egui::Key::OpenBracket),
        Keysym::parenright => Some(egui::Key::CloseBracket),

        Keysym::grave => Some(egui::Key::Backtick),
        Keysym::minus => Some(egui::Key::Minus),
        Keysym::period => Some(egui::Key::Period),
        Keysym::plus => Some(egui::Key::Plus),
        Keysym::equal => Some(egui::Key::Equals),
        Keysym::semicolon => Some(egui::Key::Semicolon),
        Keysym::apostrophe => Some(egui::Key::Quote),

        Keysym::_0 => Some(egui::Key::Num0),
        Keysym::_1 => Some(egui::Key::Num1),
        Keysym::_2 => Some(egui::Key::Num2),
        Keysym::_3 => Some(egui::Key::Num3),
        Keysym::_4 => Some(egui::Key::Num4),
        Keysym::_5 => Some(egui::Key::Num5),
        Keysym::_6 => Some(egui::Key::Num6),
        Keysym::_7 => Some(egui::Key::Num7),
        Keysym::_8 => Some(egui::Key::Num8),
        Keysym::_9 => Some(egui::Key::Num9),

        Keysym::a => Some(egui::Key::A),
        Keysym::b => Some(egui::Key::B),
        Keysym::c => Some(egui::Key::C),
        Keysym::d => Some(egui::Key::D),
        Keysym::e => Some(egui::Key::E),
        Keysym::f => Some(egui::Key::F),
        Keysym::g => Some(egui::Key::G),
        Keysym::h => Some(egui::Key::H),
        Keysym::i => Some(egui::Key::I),
        Keysym::j => Some(egui::Key::J),
        Keysym::k => Some(egui::Key::K),
        Keysym::l => Some(egui::Key::L),
        Keysym::m => Some(egui::Key::M),
        Keysym::n => Some(egui::Key::N),
        Keysym::o => Some(egui::Key::O),
        Keysym::p => Some(egui::Key::P),
        Keysym::q => Some(egui::Key::Q),
        Keysym::r => Some(egui::Key::R),
        Keysym::s => Some(egui::Key::S),
        Keysym::t => Some(egui::Key::T),
        Keysym::u => Some(egui::Key::U),
        Keysym::v => Some(egui::Key::V),
        Keysym::w => Some(egui::Key::W),
        Keysym::x => Some(egui::Key::X),
        Keysym::y => Some(egui::Key::Y),
        Keysym::z => Some(egui::Key::Z),

        Keysym::F1 => Some(egui::Key::F1),
        Keysym::F2 => Some(egui::Key::F2),
        Keysym::F3 => Some(egui::Key::F3),
        Keysym::F4 => Some(egui::Key::F4),
        Keysym::F5 => Some(egui::Key::F5),
        Keysym::F6 => Some(egui::Key::F6),
        Keysym::F7 => Some(egui::Key::F7),
        Keysym::F8 => Some(egui::Key::F8),
        Keysym::F9 => Some(egui::Key::F9),
        Keysym::F10 => Some(egui::Key::F10),
        Keysym::F11 => Some(egui::Key::F11),
        Keysym::F12 => Some(egui::Key::F12),
        Keysym::F13 => Some(egui::Key::F13),
        Keysym::F14 => Some(egui::Key::F14),
        Keysym::F15 => Some(egui::Key::F15),
        Keysym::F16 => Some(egui::Key::F16),
        Keysym::F17 => Some(egui::Key::F17),
        Keysym::F18 => Some(egui::Key::F18),
        Keysym::F19 => Some(egui::Key::F19),
        Keysym::F20 => Some(egui::Key::F20),
        Keysym::F21 => Some(egui::Key::F21),
        Keysym::F22 => Some(egui::Key::F22),
        Keysym::F23 => Some(egui::Key::F23),
        Keysym::F24 => Some(egui::Key::F24),
        Keysym::F25 => Some(egui::Key::F25),
        Keysym::F26 => Some(egui::Key::F26),
        Keysym::F27 => Some(egui::Key::F27),
        Keysym::F28 => Some(egui::Key::F28),
        Keysym::F29 => Some(egui::Key::F29),
        Keysym::F30 => Some(egui::Key::F30),
        Keysym::F31 => Some(egui::Key::F31),
        Keysym::F32 => Some(egui::Key::F32),
        Keysym::F33 => Some(egui::Key::F33),
        Keysym::F34 => Some(egui::Key::F34),
        Keysym::F35 => Some(egui::Key::F35),

        _ => None,
    }
}

fn is_printable_char(chr: char) -> bool {
    let is_in_private_use_area = '\u{e000}' <= chr && chr <= '\u{f8ff}'
        || '\u{f0000}' <= chr && chr <= '\u{ffffd}'
        || '\u{100000}' <= chr && chr <= '\u{10fffd}';

    !is_in_private_use_area && !chr.is_ascii_control()
}
