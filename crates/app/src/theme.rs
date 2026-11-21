//! The Hermes look: "Hermes Teal" colours (hermes-agent `web/src/themes/presets.ts`, theme
//! `default`: background #041c1c, midground #ffe6cb), the Hermes wallpaper, and the readable body
//! font (Inter, OFL). See THIRD_PARTY.md for asset sources.

use iced::border::Radius;
use iced::font::{Family, Weight};
use iced::widget::{button, container, scrollable, text_editor, text_input};
use iced::{Background, Border, Color, Font, Theme, color};

pub const FONT_INTER_REGULAR: &[u8] = include_bytes!("../../../assets/fonts/Inter-Regular.ttf");
pub const FONT_INTER_MEDIUM: &[u8] = include_bytes!("../../../assets/fonts/Inter-Medium.ttf");
pub const FONT_INTER_SEMIBOLD: &[u8] = include_bytes!("../../../assets/fonts/Inter-SemiBold.ttf");
pub const FONT_INTER_BOLD: &[u8] = include_bytes!("../../../assets/fonts/Inter-Bold.ttf");
pub const FONT_MONO: &[u8] = include_bytes!("../../../assets/fonts/JetBrainsMono-Regular.ttf");
pub const FONT_ICONS: &[u8] = include_bytes!("../../../assets/fonts/lucide.ttf");
pub const WALLPAPER: &[u8] = include_bytes!("../../../assets/wallpaper.jpg");
pub const APP_ICON_PNG: &[u8] = include_bytes!("../../../assets/icons/hermes-bots-256.png");

pub const BODY: Font = Font::with_name("Inter");
pub const MEDIUM: Font = Font { weight: Weight::Medium, ..BODY };
pub const SEMIBOLD: Font = Font { weight: Weight::Semibold, ..BODY };
pub const BOLD: Font = Font { weight: Weight::Bold, ..BODY };
pub const MONO: Font = Font { family: Family::Name("JetBrains Mono"), ..Font::DEFAULT };
pub const ICONS: Font = Font::with_name("lucide");

/// Hermes Teal palette.
pub mod c {
    use super::*;
    /// Window background (Hermes Teal `background`).
    pub const BG: Color = color!(0x041c1c);
    /// Sidebar: a little darker than the background.
    pub const SIDEBAR: Color = color!(0x031616);
    /// Raised surface (cards, composer).
    pub const SURFACE: Color = color!(0x0a2a2a);
    pub const SURFACE_2: Color = color!(0x103434);
    pub const HOVER: Color = color!(0x0d3030);
    pub const SELECTED: Color = color!(0x14403d);
    /// Text: Hermes cream `midground`.
    pub const TEXT: Color = color!(0xffe6cb);
    pub const TEXT_2: Color = Color { r: 1.0, g: 0.902, b: 0.796, a: 0.72 };
    pub const TEXT_3: Color = Color { r: 1.0, g: 0.902, b: 0.796, a: 0.48 };
    pub const BORDER: Color = Color { r: 1.0, g: 0.902, b: 0.796, a: 0.10 };
    pub const BORDER_STRONG: Color = Color { r: 1.0, g: 0.902, b: 0.796, a: 0.22 };
    /// Accent: the Hermes warm glow (rgba(255,189,56)).
    pub const ACCENT: Color = color!(0xffbd38);
    pub const ACCENT_SOFT: Color = Color { r: 1.0, g: 0.741, b: 0.22, a: 0.16 };
    pub const ON_ACCENT: Color = color!(0x041c1c);
    pub const GREEN: Color = color!(0x55a583);
    pub const RED: Color = color!(0xe75e78);
    pub const AMBER: Color = color!(0xe0a14a);
    pub const USER_BUBBLE: Color = color!(0x123c3a);
}

pub fn theme() -> Theme {
    Theme::custom(
        "Hermes Teal",
        iced::theme::Palette {
            background: c::BG,
            text: c::TEXT,
            primary: c::ACCENT,
            success: c::GREEN,
            warning: c::AMBER,
            danger: c::RED,
        },
    )
}

pub fn alpha(col: Color, a: f32) -> Color {
    Color { a, ..col }
}

/// Parse `#rrggbb` (Bot colours from `ui_meta`). Falls back to the accent.
pub fn parse_hex(s: Option<&str>) -> Color {
    let s = s.unwrap_or("").trim().trim_start_matches('#');
    if s.len() == 6 && let Ok(v) = u32::from_str_radix(s, 16) {
        return Color::from_rgb8((v >> 16) as u8, (v >> 8) as u8, v as u8);
    }
    // `hsl(H S% L%)` swatches from the desktop app.
    if let Some(rest) = s.strip_prefix("hsl(").and_then(|r| r.strip_suffix(')')) {
        let parts: Vec<f32> = rest
            .split(|ch: char| ch == ' ' || ch == ',')
            .filter(|p| !p.is_empty())
            .filter_map(|p| p.trim_end_matches('%').parse().ok())
            .collect();
        if parts.len() == 3 {
            return hsl(parts[0], parts[1] / 100.0, parts[2] / 100.0);
        }
    }
    c::ACCENT
}

pub fn hsl(h: f32, s: f32, l: f32) -> Color {
    let a = s * l.min(1.0 - l);
    let f = |n: f32| {
        let k = (n + h / 30.0) % 12.0;
        l - a * (k - 3.0).min(9.0 - k).clamp(-1.0, 1.0)
    };
    Color::from_rgb(f(0.0), f(8.0), f(4.0))
}

/// A stable colour for a Bot without a stored colour (12-swatch wheel, like Hermes).
pub fn color_for_name(name: &str) -> Color {
    let h = name.bytes().fold(0u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32));
    hsl((h % 12) as f32 * 30.0, 0.68, 0.58)
}

// ---------- container styles ----------

pub fn sidebar(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(alpha(c::SIDEBAR, 0.94))),
        border: Border { color: c::BORDER, width: 0.0, radius: Radius::default() },
        text_color: Some(c::TEXT),
        ..Default::default()
    }
}

pub fn pane(_: &Theme) -> container::Style {
    container::Style { background: Some(Background::Color(alpha(c::BG, 0.80))), text_color: Some(c::TEXT), ..Default::default() }
}

pub fn panel_right(_: &Theme) -> container::Style {
    container::Style { background: Some(Background::Color(alpha(c::SIDEBAR, 0.92))), text_color: Some(c::TEXT), ..Default::default() }
}

pub fn card(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(c::SURFACE)),
        border: Border { color: c::BORDER, width: 1.0, radius: 12.0.into() },
        text_color: Some(c::TEXT),
        ..Default::default()
    }
}

pub fn card_accent(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(c::SURFACE)),
        border: Border { color: alpha(c::ACCENT, 0.55), width: 1.0, radius: 12.0.into() },
        text_color: Some(c::TEXT),
        ..Default::default()
    }
}

pub fn dialog(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(color!(0x072424))),
        border: Border { color: c::BORDER_STRONG, width: 1.0, radius: 14.0.into() },
        text_color: Some(c::TEXT),
        ..Default::default()
    }
}

pub fn scrim(_: &Theme) -> container::Style {
    container::Style { background: Some(Background::Color(alpha(Color::BLACK, 0.55))), ..Default::default() }
}

pub fn user_bubble(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(c::USER_BUBBLE)),
        border: Border { color: c::BORDER, width: 1.0, radius: 14.0.into() },
        text_color: Some(c::TEXT),
        ..Default::default()
    }
}

pub fn composer(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(c::SURFACE)),
        border: Border { color: c::BORDER_STRONG, width: 1.0, radius: 14.0.into() },
        text_color: Some(c::TEXT),
        ..Default::default()
    }
}

pub fn banner(col: Color) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: Some(Background::Color(alpha(col, 0.16))),
        border: Border { color: alpha(col, 0.5), width: 1.0, radius: 10.0.into() },
        text_color: Some(c::TEXT),
        ..Default::default()
    }
}

pub fn solid(col: Color, radius: f32) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: Some(Background::Color(col)),
        border: Border { radius: radius.into(), ..Default::default() },
        ..Default::default()
    }
}

pub fn pill(col: Color) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: Some(Background::Color(alpha(col, 0.18))),
        border: Border { color: alpha(col, 0.45), width: 1.0, radius: 999.0.into() },
        text_color: Some(col),
        ..Default::default()
    }
}

pub fn floating_pill(_: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(alpha(color!(0x021212), 0.94))),
        border: Border { color: c::BORDER_STRONG, width: 1.0, radius: 999.0.into() },
        text_color: Some(c::TEXT),
        ..Default::default()
    }
}

pub fn screen_frame(control: bool) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: Some(Background::Color(Color::BLACK)),
        border: Border {
            color: if control { c::ACCENT } else { c::BORDER },
            width: if control { 2.0 } else { 1.0 },
            radius: 10.0.into(),
        },
        ..Default::default()
    }
}

// ---------- button styles ----------

pub fn btn_primary(_: &Theme, s: button::Status) -> button::Style {
    let bg = match s {
        button::Status::Hovered => color!(0xffc95c),
        button::Status::Pressed => color!(0xe8a82a),
        button::Status::Disabled => alpha(c::ACCENT, 0.35),
        button::Status::Active => c::ACCENT,
    };
    button::Style {
        background: Some(Background::Color(bg)),
        text_color: c::ON_ACCENT,
        border: Border { radius: 10.0.into(), ..Default::default() },
        ..Default::default()
    }
}

pub fn btn_secondary(_: &Theme, s: button::Status) -> button::Style {
    let bg = match s {
        button::Status::Hovered => c::SURFACE_2,
        button::Status::Pressed => c::SELECTED,
        _ => c::SURFACE,
    };
    button::Style {
        background: Some(Background::Color(bg)),
        text_color: if s == button::Status::Disabled { c::TEXT_3 } else { c::TEXT },
        border: Border { color: c::BORDER_STRONG, width: 1.0, radius: 10.0.into() },
        ..Default::default()
    }
}

pub fn btn_danger(_: &Theme, s: button::Status) -> button::Style {
    let bg = match s {
        button::Status::Hovered => alpha(c::RED, 0.28),
        _ => alpha(c::RED, 0.16),
    };
    button::Style {
        background: Some(Background::Color(bg)),
        text_color: c::RED,
        border: Border { color: alpha(c::RED, 0.5), width: 1.0, radius: 10.0.into() },
        ..Default::default()
    }
}

pub fn btn_ghost(_: &Theme, s: button::Status) -> button::Style {
    let bg = match s {
        button::Status::Hovered => Some(Background::Color(c::HOVER)),
        button::Status::Pressed => Some(Background::Color(c::SELECTED)),
        _ => None,
    };
    button::Style {
        background: bg,
        text_color: if s == button::Status::Disabled { c::TEXT_3 } else { c::TEXT_2 },
        border: Border { radius: 8.0.into(), ..Default::default() },
        ..Default::default()
    }
}

pub fn btn_row(selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, s| {
        let bg = if selected {
            Some(Background::Color(c::SELECTED))
        } else {
            match s {
                button::Status::Hovered => Some(Background::Color(c::HOVER)),
                button::Status::Pressed => Some(Background::Color(c::SELECTED)),
                _ => None,
            }
        };
        button::Style {
            background: bg,
            text_color: c::TEXT,
            border: Border { radius: 10.0.into(), ..Default::default() },
            ..Default::default()
        }
    }
}

pub fn btn_tab(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, s| button::Style {
        background: if active {
            Some(Background::Color(c::SELECTED))
        } else if s == button::Status::Hovered {
            Some(Background::Color(c::HOVER))
        } else {
            None
        },
        text_color: if active { c::TEXT } else { c::TEXT_2 },
        border: Border { radius: 8.0.into(), ..Default::default() },
        ..Default::default()
    }
}

// ---------- inputs ----------

pub fn input(_: &Theme, s: text_input::Status) -> text_input::Style {
    let focused = matches!(s, text_input::Status::Focused { .. });
    text_input::Style {
        background: Background::Color(color!(0x021414)),
        border: Border { color: if focused { alpha(c::ACCENT, 0.8) } else { c::BORDER_STRONG }, width: 1.0, radius: 10.0.into() },
        icon: c::TEXT_3,
        placeholder: c::TEXT_3,
        value: c::TEXT,
        selection: alpha(c::ACCENT, 0.35),
    }
}

pub fn editor(_: &Theme, _s: text_editor::Status) -> text_editor::Style {
    text_editor::Style {
        background: Background::Color(c::SURFACE),
        border: Border { width: 0.0, ..Default::default() },
        placeholder: c::TEXT_3,
        value: c::TEXT,
        selection: alpha(c::ACCENT, 0.35),
    }
}

pub fn scroll(_: &Theme, s: scrollable::Status) -> scrollable::Style {
    let mut st = scrollable::default(&theme(), s);
    let rail = |_r: scrollable::Rail| scrollable::Rail {
        background: None,
        border: Border::default(),
        scroller: scrollable::Scroller {
            background: Background::Color(alpha(c::TEXT, 0.14)),
            border: Border { radius: 4.0.into(), ..Default::default() },
        },

    };
    st.vertical_rail = rail(st.vertical_rail);
    st.horizontal_rail = rail(st.horizontal_rail);
    st
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_and_hsl() {
        assert_eq!(parse_hex(Some("#ff0000")), Color::from_rgb8(255, 0, 0));
        assert_eq!(parse_hex(Some("bad")), c::ACCENT);
        assert_eq!(parse_hex(None), c::ACCENT);
        let red = hsl(0.0, 1.0, 0.5);
        assert!((red.r - 1.0).abs() < 1e-3 && red.g.abs() < 1e-3);
        let _ = color_for_name("scout");
    }
}
