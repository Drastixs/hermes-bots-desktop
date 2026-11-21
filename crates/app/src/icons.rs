//! Lucide icon codepoints (assets/fonts/lucide.ttf, ISC licence).

use iced::widget::{Text, text};

pub const SETTINGS: char = '\u{e154}';
pub const PLUS: char = '\u{e13d}';
pub const MONITOR: char = '\u{e11d}';
pub const SEND: char = '\u{e4f2}';
pub const STOP: char = '\u{e167}';
pub const CHECK: char = '\u{e06c}';
pub const X: char = '\u{e1b2}';
pub const CHEVRON_RIGHT: char = '\u{e06f}';
pub const CHEVRON_DOWN: char = '\u{e06d}';
pub const LOADER: char = '\u{e10a}';
pub const WIFI_OFF: char = '\u{e1af}';
pub const ALERT: char = '\u{e193}';
pub const LOG_OUT: char = '\u{e10e}';
pub const MAXIMIZE: char = '\u{e113}';
pub const MINIMIZE: char = '\u{e11b}';
pub const POINTER: char = '\u{e1c3}';
pub const HAND: char = '\u{e1d7}';
pub const REFRESH: char = '\u{e145}';
pub const SEARCH: char = '\u{e151}';
pub const BOT: char = '\u{e1bb}';
pub const CIRCLE_ALERT: char = '\u{e077}';
pub const CIRCLE_CHECK: char = '\u{e07c}';
pub const WRENCH: char = '\u{e1b1}';
pub const GLOBE: char = '\u{e0e8}';
pub const TERMINAL: char = '\u{e181}';
pub const FILE: char = '\u{e0cc}';
pub const SPARKLES: char = '\u{e412}';
pub const EYE: char = '\u{e0ba}';
pub const LOG_IN: char = '\u{e10d}';
pub const ARROW_LEFT: char = '\u{e048}';
pub const KEY: char = '\u{e4a3}';
pub const USER: char = '\u{e19f}';
pub const BRAIN: char = '\u{e3c6}';
pub const UNDO: char = '\u{e2a1}';

pub fn icon<'a>(c: char) -> Text<'a> {
    text(c.to_string()).font(crate::theme::ICONS).size(16).line_height(1.0)
}

pub fn for_tool(name: &str) -> char {
    match name {
        "web_search" | "web_extract" => SEARCH,
        "browser_navigate" | "browser_click" | "browser_type" => GLOBE,
        "computer_use" => MONITOR,
        "terminal" | "process" => TERMINAL,
        "read_file" | "write_file" | "patch" | "search_files" => FILE,
        "message_agent" => BOT,
        _ => WRENCH,
    }
}
