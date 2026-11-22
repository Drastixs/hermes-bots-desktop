//! An input layer over the Bot Screen. It turns keyboard and mouse events into RFB messages in
//! the order they happen (one widget handles both, so a click never overtakes earlier keys).

use crate::app::Msg;
use hermes_api::rfb;
use iced::widget::canvas::{self, Action, Event, Geometry};
use iced::{Rectangle, Renderer, Theme, mouse};

pub struct ScreenInput {
    pub fb: (u16, u16),
    pub enabled: bool,
}

#[derive(Default)]
pub struct State {
    buttons: u8,
    last: (u16, u16),
}

fn mask(b: mouse::Button) -> u8 {
    match b {
        mouse::Button::Left => 1,
        mouse::Button::Middle => 2,
        mouse::Button::Right => 4,
        _ => 0,
    }
}

impl canvas::Program<Msg> for ScreenInput {
    type State = State;

    fn update(&self, st: &mut State, event: &Event, bounds: Rectangle, cursor: mouse::Cursor) -> Option<Action<Msg>> {
        if !self.enabled {
            return None;
        }
        let map = |cursor: mouse::Cursor| cursor.position_in(bounds).and_then(|p| crate::model::view_to_fb((p.x, p.y), (bounds.width, bounds.height), self.fb));
        let out = match event {
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let (x, y) = map(cursor)?;
                st.last = (x, y);
                vec![rfb::pointer_event(st.buttons, x, y)]
            }
            Event::Mouse(mouse::Event::ButtonPressed(b)) => {
                let (x, y) = map(cursor)?;
                st.last = (x, y);
                st.buttons |= mask(*b);
                vec![rfb::pointer_event(st.buttons, x, y)]
            }
            Event::Mouse(mouse::Event::ButtonReleased(b)) => {
                st.buttons &= !mask(*b);
                vec![rfb::pointer_event(st.buttons, st.last.0, st.last.1)]
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                map(cursor)?;
                let dy = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => *y,
                    mouse::ScrollDelta::Pixels { y, .. } => *y / 40.0,
                };
                let bit = if dy > 0.0 { 8 } else { 16 };
                let (x, y) = st.last;
                vec![rfb::pointer_event(st.buttons | bit, x, y), rfb::pointer_event(st.buttons, x, y)]
            }
            Event::Keyboard(iced::keyboard::Event::KeyPressed { key, text, modifiers, .. }) => {
                let ks = crate::app::keysym_for(key, text.as_deref(), *modifiers)?;
                vec![rfb::key_event(true, ks), rfb::key_event(false, ks)]
            }
            _ => return None,
        };
        Some(Action::publish(Msg::ScreenBytes(out)).and_capture())
    }

    fn draw(&self, _: &State, _: &Renderer, _: &Theme, _: Rectangle, _: mouse::Cursor) -> Vec<Geometry> {
        vec![]
    }

    fn mouse_interaction(&self, _: &State, bounds: Rectangle, cursor: mouse::Cursor) -> mouse::Interaction {
        if self.enabled && cursor.is_over(bounds) { mouse::Interaction::Crosshair } else { mouse::Interaction::default() }
    }
}
