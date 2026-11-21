//! Bot avatars: a coloured rounded square with a simple face (two eyes), drawn with canvas.
//! Status shows as a small dot and, while working, as raised eyes with three dots (like Hermes).

use crate::model::BotStatus;
use crate::theme::{self, c};
use iced::widget::canvas::{self, Frame, Geometry, Path, Stroke};
use iced::{Color, Element, Length, Point, Rectangle, Renderer, Size, Theme, mouse};

pub struct Avatar {
    pub color: Color,
    pub status: BotStatus,
    pub phase: f32,
    pub show_dot: bool,
}

impl<Message> canvas::Program<Message> for Avatar {
    type State = ();

    fn draw(&self, _: &(), renderer: &Renderer, _: &Theme, bounds: Rectangle, _: mouse::Cursor) -> Vec<Geometry> {
        let mut f = Frame::new(renderer, bounds.size());
        let s = bounds.width.min(bounds.height);
        let r = s * 0.30;
        let body = Path::rounded_rectangle(Point::ORIGIN, Size::new(s, s), r.into());
        f.fill(&body, self.color);
        // A soft inner shade at the bottom.
        let shade = Path::rounded_rectangle(Point::new(0.0, s * 0.62), Size::new(s, s * 0.38), r.into());
        f.fill(&shade, theme::alpha(Color::BLACK, 0.10));
        let working = matches!(self.status, BotStatus::Thinking | BotStatus::Working(_));
        let lift = if working { s * 0.06 } else { 0.0 };
        let eye_r = s * 0.085;
        let ey = s * 0.46 - lift;
        let face = c::BG;
        f.fill(&Path::circle(Point::new(s * 0.36, ey), eye_r), face);
        f.fill(&Path::circle(Point::new(s * 0.64, ey), eye_r), face);
        if working {
            for i in 0..3 {
                let a = ((self.phase * 6.0 + i as f32 * 0.9).sin() * 0.5 + 0.5).clamp(0.25, 1.0);
                f.fill(&Path::circle(Point::new(s * (0.38 + 0.12 * i as f32), s * 0.72), s * 0.04), theme::alpha(face, a));
            }
        } else {
            let mouth = Path::line(Point::new(s * 0.40, s * 0.70), Point::new(s * 0.60, s * 0.70));
            f.stroke(&mouth, Stroke::default().with_color(theme::alpha(face, 0.8)).with_width((s * 0.05).max(1.2)));
        }
        if self.show_dot {
            let dot = match &self.status {
                BotStatus::Idle => None,
                BotStatus::Thinking | BotStatus::Working(_) => Some(c::ACCENT),
                BotStatus::NeedsYou(_) => Some(c::AMBER),
                BotStatus::Error => Some(c::RED),
            };
            if let Some(col) = dot {
                let d = s * 0.30;
                let center = Point::new(s - d * 0.45, s - d * 0.45);
                f.fill(&Path::circle(center, d * 0.62), c::SIDEBAR);
                f.fill(&Path::circle(center, d * 0.42), col);
            }
        }
        vec![f.into_geometry()]
    }
}

pub fn avatar<'a, M: 'a>(color: Color, status: BotStatus, phase: f32, size: f32, show_dot: bool) -> Element<'a, M> {
    canvas::Canvas::new(Avatar { color, status, phase, show_dot })
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
        .into()
}
