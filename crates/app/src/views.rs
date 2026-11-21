//! All views. Layout follows docs/ux-reference.md section 3 (roster sidebar, per-Bot chat,
//! details pane with the Bot's computer, full-screen Bot Screen with take-over).

use crate::app::*;
use crate::avatar::avatar;
use crate::icons::{self, icon};
use crate::model::*;
use crate::net::ConnState;
use crate::theme::{self, c};
use iced::widget::{
    Space, button, center, column, container, image, markdown, mouse_area, opaque, row, scrollable, stack, text,
    text_editor, text_input,
};
use iced::{Alignment, Color, ContentFit, Element, Fill, Length, Padding, Shrink};

type El<'a> = Element<'a, Msg>;

const SIDEBAR_W: f32 = 272.0;
const DETAILS_W: f32 = 360.0;
const TRANSCRIPT_MAX: f32 = 760.0;

fn t<'a>(s: impl text::IntoFragment<'a>) -> text::Text<'a> {
    text(s).size(14).color(c::TEXT)
}
fn muted<'a>(s: impl text::IntoFragment<'a>) -> text::Text<'a> {
    text(s).size(13).color(c::TEXT_2)
}
fn faint<'a>(s: impl text::IntoFragment<'a>) -> text::Text<'a> {
    text(s).size(12).color(c::TEXT_3)
}
fn heading<'a>(s: impl text::IntoFragment<'a>, size: f32) -> text::Text<'a> {
    text(s).size(size).font(theme::SEMIBOLD).color(c::TEXT)
}

fn icon_btn<'a>(ic: char, msg: Msg, tip: &'a str) -> El<'a> {
    let b = button(container(icon(ic).size(17).color(c::TEXT_2)).center(Length::Fixed(32.0)))
        .padding(0)
        .style(theme::btn_ghost)
        .on_press(msg);
    iced::widget::tooltip(b, container(text(tip).size(12)).padding([4, 8]).style(theme::dialog), iced::widget::tooltip::Position::Bottom).into()
}

fn primary<'a>(label: &'a str, msg: Option<Msg>) -> button::Button<'a, Msg> {
    button(text(label).size(14).font(theme::SEMIBOLD)).padding([9, 16]).style(theme::btn_primary).on_press_maybe(msg)
}
fn secondary<'a>(label: impl text::IntoFragment<'a>, msg: Option<Msg>) -> button::Button<'a, Msg> {
    button(text(label).size(14).font(theme::MEDIUM)).padding([9, 16]).style(theme::btn_secondary).on_press_maybe(msg)
}

fn spinner<'a>(phase: f32) -> El<'a> {
    // A rotating set of dots (text-based to keep it cheap).
    let frames = ['◐', '◓', '◑', '◒'];
    let i = ((phase * 4.0) as usize) % frames.len();
    text(frames[i].to_string()).size(14).color(c::ACCENT).into()
}

pub fn view(app: &App) -> El<'_> {
    let content: El = match &app.phase {
        Phase::SignIn(st) => sign_in(app, st),
        Phase::Main => main_view(app),
    };
    let mut layers: Vec<El> = vec![];
    // Hermes wallpaper, dimmed, under everything.
    if app.settings.show_wallpaper {
        layers.push(
            container(image(app.wallpaper.clone()).content_fit(ContentFit::Cover).width(Fill).height(Fill).opacity(0.55_f32))
                .width(Fill)
                .height(Fill)
                .style(theme::solid(c::BG, 0.0))
                .into(),
        );
    } else {
        layers.push(container(Space::new()).width(Fill).height(Fill).style(theme::solid(c::BG, 0.0)).into());
    }
    layers.push(content);
    if let Some(pane) = app.settings_open {
        layers.push(modal(settings_view(app, pane), Msg::CloseSettings));
    }
    if let Some(f) = &app.create {
        layers.push(modal(create_view(f), Msg::CreateCancel));
    }
    if let Some(toast) = &app.toast {
        layers.push(
            container(container(row![icon(icons::CIRCLE_CHECK).color(c::GREEN), t(toast.as_str())].spacing(8).align_y(Alignment::Center)).padding([10, 16]).style(theme::floating_pill))
                .width(Fill)
                .height(Fill)
                .align_x(Alignment::Center)
                .align_y(Alignment::End)
                .padding(Padding { bottom: 90.0, ..Default::default() })
                .into(),
        );
    }
    stack(layers).width(Fill).height(Fill).into()
}

fn modal<'a>(content: El<'a>, on_close: Msg) -> El<'a> {
    opaque(mouse_area(center(opaque(content)).style(theme::scrim)).on_press(on_close))
}

// ---------------------------------------------------------------- sign-in

fn sign_in<'a>(app: &'a App, st: &'a SignInState) -> El<'a> {
    let busy = st.checking || st.waiting_browser;
    let mut col = column![
        row![
            container(image(image::Handle::from_bytes(theme::APP_ICON_PNG)).width(44).height(44)),
            column![heading("Hermes Bots", 26.0), muted("Your team of always-on Bots.")].spacing(2)
        ]
        .spacing(14)
        .align_y(Alignment::Center),
        Space::new().height(8),
        text("Gateway address").size(13).font(theme::MEDIUM).color(c::TEXT_2),
        text_input("https://hermes.example.net", &st.url_input)
            .id("gateway-url")
            .on_input_maybe((!busy).then_some(Msg::UrlInput as fn(String) -> Msg))
            .on_submit(Msg::SignInPressed)
            .padding([10, 12])
            .size(15)
            .style(theme::input),
        faint("The address of your Hermes Agent gateway. The app only talks to its API."),
    ]
    .spacing(8)
    .width(400);
    if st.expired_notice {
        col = col.push(
            container(row![icon(icons::KEY).color(c::AMBER), muted("Your session expired. Sign in again to continue.")].spacing(8).align_y(Alignment::Center))
                .padding([8, 12])
                .width(Fill)
                .style(theme::banner(c::AMBER)),
        );
    }
    col = col.push(Space::new().height(6));
    if st.waiting_browser {
        col = col.push(
            container(
                column![
                    row![spinner(app.phase_t), t(st.status.clone().unwrap_or_default())].spacing(10).align_y(Alignment::Center),
                    faint("Sign in on the gateway page in your browser. Then come back here."),
                    row![secondary("Open browser again", Some(Msg::OpenBrowserAgain)), secondary("Cancel", Some(Msg::CancelSignIn))].spacing(8),
                ]
                .spacing(10),
            )
            .padding(14)
            .width(Fill)
            .style(theme::card),
        );
    } else {
        let label = if st.checking { "Checking…" } else { "Sign in with browser" };
        col = col.push(
            button(row![icon(icons::LOG_IN).color(c::ON_ACCENT), text(label).size(15).font(theme::SEMIBOLD)].spacing(8).align_y(Alignment::Center))
                .width(Fill)
                .padding([11, 16])
                .style(theme::btn_primary)
                .on_press_maybe((!busy).then_some(Msg::SignInPressed)),
        );
    }
    if let Some(e) = &st.error {
        col = col.push(
            container(row![icon(icons::CIRCLE_ALERT).color(c::RED), text(e.as_str()).size(13).color(c::TEXT)].spacing(8).align_y(Alignment::Center))
                .padding([8, 12])
                .width(Fill)
                .style(theme::banner(c::RED)),
        );
    }
    col = col.push(Space::new().height(4)).push(faint("Your sign-in token is kept in the system keyring, never in a file."));
    center(container(col).padding(32).style(theme::dialog)).into()
}

// ---------------------------------------------------------------- main

fn main_view(app: &App) -> El<'_> {
    let body: El = if app.screen.mode != ScreenMode::Closed {
        row![sidebar(app), screen_full(app)].into()
    } else {
        let mut r = row![sidebar(app), chat_view(app)];
        if app.details_open {
            r = r.push(details(app));
        }
        r.into()
    };
    let mut col = column![];
    match &app.conn {
        ConnState::Offline { reason, retry_in } => {
            col = col.push(
                container(
                    row![
                        icon(icons::WIFI_OFF).color(c::TEXT_2),
                        t("You're offline."),
                        muted(format!("{reason}. Trying again in {retry_in} s. The app reconnects on its own.")),
                    ]
                    .spacing(10)
                    .align_y(Alignment::Center),
                )
                .padding([7, 14])
                .width(Fill)
                .style(theme::solid(Color::from_rgb8(0x3a, 0x2a, 0x12), 0.0)),
            )
        }
        ConnState::Connecting if app.bots_loaded => {
            col = col.push(
                container(row![spinner(app.phase_t), muted("Reconnecting…")].spacing(10).align_y(Alignment::Center))
                    .padding([7, 14])
                    .width(Fill)
                    .style(theme::solid(c::SURFACE, 0.0)),
            )
        }
        _ => {}
    }
    col.push(body).into()
}

// ---------------------------------------------------------------- sidebar

fn sidebar(app: &App) -> El<'_> {
    let header = row![
        container(image(image::Handle::from_bytes(theme::APP_ICON_PNG)).width(22).height(22)),
        text("Bots").size(17).font(theme::SEMIBOLD).color(c::TEXT),
        Space::new().width(Fill),
        icon_btn(icons::PLUS, Msg::NewBot, "New Bot (Ctrl+N)"),
    ]
    .spacing(10)
    .align_y(Alignment::Center)
    .padding([0, 4]);
    let search = text_input("Search Bots", &app.search)
        .on_input(Msg::Search)
        .padding([7, 10])
        .size(13)
        .icon(text_input::Icon { font: theme::ICONS, code_point: icons::SEARCH, size: Some(14.into()), spacing: 8.0, side: text_input::Side::Left })
        .style(theme::input);

    let bots = app.visible_bots();
    let mut list = column![].spacing(2);
    if !app.bots_loaded {
        for _ in 0..6 {
            list = list.push(
                container(row![container(Space::new()).width(36).height(36).style(theme::solid(c::SURFACE, 11.0)), column![container(Space::new()).width(110).height(10).style(theme::solid(c::SURFACE, 4.0)), container(Space::new()).width(170).height(8).style(theme::solid(c::SURFACE, 4.0))].spacing(8)].spacing(12).align_y(Alignment::Center))
                    .padding([10, 10]),
            );
        }
    } else if bots.is_empty() {
        list = list.push(container(muted(if app.search.is_empty() { "No Bots yet." } else { "No Bot matches your search." })).padding(12));
    }
    let needs: Vec<&&Bot> = bots.iter().filter(|b| matches!(app.status_of(&b.name), BotStatus::NeedsYou(_))).collect();
    if !needs.is_empty() {
        list = list.push(container(faint("NEEDS YOU")).padding(Padding { top: 6.0, bottom: 4.0, left: 10.0, right: 0.0 }));
        for b in &needs {
            list = list.push(bot_row(app, b));
        }
        list = list.push(container(faint("ALL BOTS")).padding(Padding { top: 10.0, bottom: 4.0, left: 10.0, right: 0.0 }));
    }
    for b in bots.iter().filter(|b| !matches!(app.status_of(&b.name), BotStatus::NeedsYou(_))) {
        list = list.push(bot_row(app, b));
    }

    let me = app.me.as_ref().map(|m| if m.display_name.is_empty() { m.user_id.clone() } else { m.display_name.clone() }).unwrap_or_else(|| "Signed in".into());
    let conn_dot = match app.conn {
        ConnState::Online => c::GREEN,
        ConnState::Connecting => c::AMBER,
        ConnState::Offline { .. } => c::RED,
    };
    let footer = row![
        container(text(me.chars().next().unwrap_or('U').to_uppercase().to_string()).size(13).font(theme::SEMIBOLD).color(c::ON_ACCENT))
            .center(Length::Fixed(28.0))
            .style(theme::solid(c::TEXT_2, 14.0)),
        column![text(me).size(13).font(theme::MEDIUM).color(c::TEXT), row![container(Space::new()).width(7).height(7).style(theme::solid(conn_dot, 4.0)), faint(gateway_host(app))].spacing(6).align_y(Alignment::Center)].spacing(2),
        Space::new().width(Fill),
        icon_btn(icons::SETTINGS, Msg::OpenSettings(SettingsPane::General), "Settings (Ctrl+,)"),
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    container(
        column![
            header,
            search,
            scrollable(list).height(Fill).style(theme::scroll),
            container(Space::new()).width(Fill).height(1).style(theme::solid(c::BORDER, 0.0)),
            footer
        ]
        .spacing(12),
    )
    .padding([14, 12])
    .width(SIDEBAR_W)
    .height(Fill)
    .style(theme::sidebar)
    .into()
}

fn gateway_host(app: &App) -> String {
    app.session.as_ref().and_then(|s| s.gateway.host_str().map(str::to_string)).unwrap_or_default()
}

fn bot_row<'a>(app: &'a App, b: &'a Bot) -> El<'a> {
    let status = app.status_of(&b.name);
    let selected = app.selected.as_deref() == Some(&b.name);
    let unread = app.unread.get(&b.name).copied().unwrap_or(false);
    let (line, line_color) = match &status {
        BotStatus::Thinking => ("Thinking…".to_string(), c::ACCENT),
        BotStatus::Working(w) => (format!("{w}…"), c::ACCENT),
        BotStatus::NeedsYou(w) => (w.clone(), c::AMBER),
        BotStatus::Error => ("Something went wrong".into(), c::RED),
        BotStatus::Idle => (one_line(&b.preview, 34), c::TEXT_3),
    };
    let line = if line.is_empty() { one_line(&b.description, 34) } else { line };
    let mut name_row = row![text(b.title.as_str()).size(14).font(if unread { theme::BOLD } else { theme::MEDIUM }).color(c::TEXT)].spacing(6).align_y(Alignment::Center);
    if b.is_default {
        name_row = name_row.push(faint("★"));
    }
    name_row = name_row.push(Space::new().width(Fill));
    if unread {
        name_row = name_row.push(container(Space::new()).width(8).height(8).style(theme::solid(c::ACCENT, 4.0)));
    }
    let content = row![
        avatar(b.color, status.clone(), app.phase_t, 36.0, true),
        column![name_row, text(line).size(12).color(line_color).wrapping(text::Wrapping::None)].spacing(3).width(Fill),
    ]
    .spacing(12)
    .align_y(Alignment::Center);
    button(content).width(Fill).padding([9, 10]).style(theme::btn_row(selected)).on_press(Msg::SelectBot(b.name.clone())).into()
}

// ---------------------------------------------------------------- chat

fn chat_view(app: &App) -> El<'_> {
    let Some(bot) = app.selected_bot() else {
        let msg: El = if app.bots_loaded && app.bots.is_empty() {
            column![avatar(c::ACCENT, BotStatus::Idle, 0.0, 72.0, false), heading("Create your first Bot", 20.0), muted("A Bot has one job, its own memory and its own computer."), primary("New Bot", Some(Msg::NewBot))]
                .spacing(14)
                .align_x(Alignment::Center)
                .into()
        } else {
            column![spinner(app.phase_t), muted("Loading your Bots…")].spacing(10).align_x(Alignment::Center).into()
        };
        return container(center(msg)).width(Fill).height(Fill).style(theme::pane).into();
    };
    let status = app.status_of(&bot.name);
    let chat = app.chats.get(&bot.name);
    let screen_running = app.screen.status.as_ref().is_some_and(|s| s.running && s.profile == bot.name);
    let status_line: El = match &status {
        BotStatus::Thinking => text("Thinking…").size(12).color(c::ACCENT).into(),
        BotStatus::Working(w) => text(format!("{w}…")).size(12).color(c::ACCENT).into(),
        BotStatus::NeedsYou(w) => text(w.clone()).size(12).color(c::AMBER).into(),
        BotStatus::Error => text("Something went wrong").size(12).color(c::RED).into(),
        BotStatus::Idle => faint(one_line(&bot.description, 80)).into(),
    };
    let header = row![
        button(row![avatar(bot.color, status.clone(), app.phase_t, 34.0, false), column![text(bot.title.as_str()).size(16).font(theme::SEMIBOLD).color(c::TEXT), status_line].spacing(1)].spacing(12).align_y(Alignment::Center))
            .padding([4, 6])
            .style(theme::btn_ghost)
            .on_press(Msg::ToggleDetails),
        Space::new().width(Fill),
        computer_button(screen_running, app.details_open),
    ]
    .align_y(Alignment::Center)
    .padding([10, 18]);

    let body: El = match chat {
        Some(ch) if ch.error.is_some() && ch.session_id.is_none() => center(
            column![
                icon(icons::CIRCLE_ALERT).size(28).color(c::RED),
                heading(format!("Couldn't open {}'s chat", bot.title), 16.0),
                muted(ch.error.clone().unwrap_or_default()),
                secondary("Retry", Some(Msg::RetryChat))
            ]
            .spacing(10)
            .align_x(Alignment::Center),
        )
        .into(),
        Some(ch) if !ch.items.is_empty() => transcript(app, bot, ch),
        Some(ch) if ch.loading && ch.session_id.is_none() => center(column![spinner(app.phase_t), muted("Opening Bot Chat…")].spacing(10).align_x(Alignment::Center)).into(),
        _ => center(
            column![avatar(bot.color, status.clone(), app.phase_t, 84.0, false), heading(bot.title.as_str(), 20.0), muted(bot.description.as_str()).align_x(Alignment::Center)]
                .spacing(12)
                .max_width(420)
                .align_x(Alignment::Center),
        )
        .into(),
    };
    let busy = chat.is_some_and(|c| c.busy);
    let can_send = app.online() && chat.is_some_and(|c| c.session_id.is_some());
    let placeholder = if !app.online() {
        "You're offline".to_string()
    } else {
        format!("Message {}", bot.title)
    };
    let editor = text_editor(&app.composer)
        .id("composer")
        .placeholder(placeholder)
        .on_action(Msg::Composer)
        .padding([10, 4])
        .size(14)
        .min_height(24)
        .max_height(200)
        .style(theme::editor)
        .key_binding(|kp| {
            if matches!(kp.key, iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter)) && !kp.modifiers.shift() {
                Some(text_editor::Binding::Custom(Msg::Send))
            } else {
                text_editor::Binding::from_key_press(kp)
            }
        });
    let action: El = if busy {
        button(container(icon(icons::STOP).size(14).color(c::ON_ACCENT)).center(Length::Fixed(34.0))).padding(0).style(theme::btn_primary).on_press(Msg::Stop).into()
    } else {
        button(container(icon(icons::SEND).size(15).color(c::ON_ACCENT)).center(Length::Fixed(34.0)))
            .padding(0)
            .style(theme::btn_primary)
            .on_press_maybe((can_send && !app.composer.text().trim().is_empty()).then_some(Msg::Send))
            .into()
    };
    let composer = container(row![editor, action].spacing(8).align_y(Alignment::End)).padding(Padding { top: 4.0, bottom: 6.0, left: 12.0, right: 6.0 }).style(theme::composer).max_width(TRANSCRIPT_MAX);
    let mut bottom = column![].spacing(8).align_x(Alignment::Center);
    if busy {
        let txt = chat.map(|c| c.status_text.clone()).filter(|s| !s.is_empty()).unwrap_or_else(|| "Working".into());
        bottom = bottom.push(container(row![avatar(bot.color, BotStatus::Thinking, app.phase_t, 16.0, false), faint(format!("{txt}…"))].spacing(8).align_y(Alignment::Center)).max_width(TRANSCRIPT_MAX).width(Fill));
    }
    bottom = bottom.push(composer);
    container(column![header, container(Space::new()).width(Fill).height(1).style(theme::solid(c::BORDER, 0.0)), container(body).height(Fill).width(Fill), container(bottom).padding(Padding { top: 6.0, bottom: 16.0, left: 24.0, right: 24.0 }).center_x(Fill)])
        .width(Fill)
        .height(Fill)
        .style(theme::pane)
        .into()
}

fn computer_button<'a>(running: bool, open: bool) -> El<'a> {
    let col = if running { c::ACCENT } else { c::TEXT_2 };
    let b = button(row![icon(icons::MONITOR).color(col), text(if running { "Computer · live" } else { "Computer" }).size(13).color(col)].spacing(7).align_y(Alignment::Center))
        .padding([7, 12])
        .style(theme::btn_tab(open))
        .on_press(Msg::ToggleDetails);
    b.into()
}

fn transcript<'a>(app: &'a App, bot: &'a Bot, ch: &'a Chat) -> El<'a> {
    let mut col = column![].spacing(14).padding(Padding { top: 20.0, bottom: 20.0, left: 24.0, right: 24.0 });
    for (i, item) in ch.items.iter().enumerate() {
        col = col.push(item_view(app, bot, i, item));
    }
    scrollable(container(col.max_width(TRANSCRIPT_MAX)).center_x(Fill))
        .id("transcript")
        .height(Fill)
        .style(theme::scroll)
        .into()
}

fn md_settings() -> markdown::Settings {
    let style = markdown::Style {
        font: theme::BODY,
        inline_code_highlight: markdown::Highlight { background: iced::Background::Color(c::SURFACE_2), border: iced::border::rounded(4) },
        inline_code_padding: Padding::from([0, 3]),
        inline_code_color: c::TEXT,
        inline_code_font: theme::MONO,
        code_block_font: theme::MONO,
        link_color: c::ACCENT,
    };
    let mut s = markdown::Settings::with_text_size(14, style);
    s.h1_size = 20.into();
    s.h2_size = 18.into();
    s.h3_size = 16.into();
    s
}

fn item_view<'a>(app: &'a App, bot: &'a Bot, i: usize, item: &'a Item) -> El<'a> {
    match item {
        Item::User { text: s } => container(container(text(s.as_str()).size(14).color(c::TEXT)).padding([10, 14]).style(theme::user_bubble).max_width(560)).align_right(Fill).into(),
        Item::Assistant { text: s, streaming } => {
            let mut body = s.clone();
            if *streaming {
                body.push_str(" ▍");
            }
            let items: Vec<markdown::Item> = markdown::parse(&body).collect();
            let md: El = markdown::view(&items, md_settings()).map(|_| Msg::Noop);
            // markdown::view borrows `items`; render to an owned element via a column of paragraphs.
            let _ = md;
            row![avatar(bot.color, BotStatus::Idle, 0.0, 26.0, false), container(md_owned(body)).width(Fill)].spacing(12).into()
        }
        Item::Activity { tools, expanded } => activity_view(app, i, tools, *expanded),
        Item::ComputerCard { text: s, resolved } => {
            let mut col = column![
                row![icon(icons::MONITOR).color(c::ACCENT), text("Computer").size(14).font(theme::SEMIBOLD).color(c::TEXT), Space::new().width(Fill), container(text(if *resolved { "Done" } else { "Action needed" }).size(11).font(theme::SEMIBOLD)).padding([2, 8]).style(theme::pill(if *resolved { c::GREEN } else { c::AMBER }))]
                    .spacing(8)
                    .align_y(Alignment::Center),
                muted(s.as_str()),
            ]
            .spacing(8);
            if !resolved {
                col = col.push(
                    row![
                        button(row![icon(icons::HAND).color(c::ON_ACCENT), text("Take over").size(14).font(theme::SEMIBOLD)].spacing(8).align_y(Alignment::Center)).padding([8, 14]).style(theme::btn_primary).on_press(Msg::OpenScreen { take_over: true }),
                        secondary("Watch", Some(Msg::OpenScreen { take_over: false })),
                    ]
                    .spacing(8),
                );
                col = col.push(faint("The Bot never asks for passwords in chat. Type them only on its screen."));
            }
            container(col).padding(14).width(Fill).style(if *resolved { theme::card } else { theme::card_accent }).into()
        }
        Item::Approval { command, description, answer, .. } => {
            let mut col = column![
                row![icon(icons::KEY).color(c::AMBER), text("Approval needed").size(14).font(theme::SEMIBOLD).color(c::TEXT)].spacing(8).align_y(Alignment::Center),
                muted(description.as_str()),
                container(text(command.as_str()).font(theme::MONO).size(12).color(c::TEXT)).padding(10).width(Fill).style(theme::solid(Color::from_rgb8(2, 18, 18), 8.0)),
            ]
            .spacing(8);
            match answer {
                None => {
                    col = col.push(
                        row![
                            primary("Allow once", Some(Msg::Approve(i, "once".into()))),
                            secondary("Always allow", Some(Msg::Approve(i, "always".into()))),
                            button(text("Deny").size(14)).padding([9, 16]).style(theme::btn_danger).on_press(Msg::Approve(i, "deny".into()))
                        ]
                        .spacing(8),
                    )
                }
                Some(a) => col = col.push(faint(if a == "deny" { "Denied" } else { "Allowed" })),
            }
            container(col).padding(14).width(Fill).style(theme::card_accent).into()
        }
        Item::Notice { text: s, error } => {
            let col = if *error { c::RED } else { c::GREEN };
            container(row![icon(if *error { icons::CIRCLE_ALERT } else { icons::CIRCLE_CHECK }).color(col), text(s.as_str()).size(13).color(c::TEXT)].spacing(8).align_y(Alignment::Center))
                .padding([8, 12])
                .width(Fill)
                .style(theme::banner(col))
                .into()
        }
    }
}

/// Markdown rendered from an owned string. iced's markdown view borrows its items, so we keep a
/// simple, robust renderer here: paragraphs, bullets, headings, code blocks and **bold** spans.
fn md_owned<'a>(src: String) -> El<'a> {
    let mut col = column![].spacing(8);
    let mut in_code = false;
    let mut code = String::new();
    for line in src.lines() {
        if line.trim_start().starts_with("```") {
            if in_code {
                col = col.push(container(text(std::mem::take(&mut code)).font(theme::MONO).size(12).color(c::TEXT)).padding(10).width(Fill).style(theme::solid(Color::from_rgb8(2, 18, 18), 8.0)));
            }
            in_code = !in_code;
            continue;
        }
        if in_code {
            code.push_str(line);
            code.push('\n');
            continue;
        }
        let l = line.trim_end();
        if l.trim().is_empty() {
            continue;
        }
        if let Some(h) = l.strip_prefix("### ").or_else(|| l.strip_prefix("## ")).or_else(|| l.strip_prefix("# ")) {
            col = col.push(text(h.to_string()).size(16).font(theme::SEMIBOLD).color(c::TEXT));
        } else if let Some(b) = l.trim_start().strip_prefix("- ").or_else(|| l.trim_start().strip_prefix("* ")) {
            col = col.push(row![text("•").size(14).color(c::ACCENT), rich_line(b)].spacing(8));
        } else {
            col = col.push(rich_line(l));
        }
    }
    if in_code && !code.is_empty() {
        col = col.push(container(text(code).font(theme::MONO).size(12).color(c::TEXT)).padding(10).width(Fill).style(theme::solid(Color::from_rgb8(2, 18, 18), 8.0)));
    }
    col.into()
}

fn rich_line<'a>(l: &str) -> El<'a> {
    // Split on ** for bold and ` for code.
    let mut spans: Vec<iced::widget::text::Span<'a, ()>> = vec![];
    let mut bold = false;
    for (n, part) in l.split("**").enumerate() {
        if n > 0 {
            bold = !bold;
        }
        let mut code = false;
        for (m, seg) in part.split('`').enumerate() {
            if m > 0 {
                code = !code;
            }
            if seg.is_empty() {
                continue;
            }
            let mut sp = iced::widget::span(seg.to_string()).color(c::TEXT);
            if bold {
                sp = sp.font(theme::SEMIBOLD);
            }
            if code {
                sp = sp.font(theme::MONO).background(c::SURFACE_2).padding([0, 3]);
            }
            spans.push(sp);
        }
    }
    iced::widget::rich_text(spans).size(14).line_height(1.5).on_link_click(|_: ()| Msg::Noop).into()
}

fn activity_view<'a>(app: &'a App, i: usize, tools: &'a [ToolCall], expanded: bool) -> El<'a> {
    let running = tools.iter().filter(|t| t.state == ToolState::Running).count();
    let failed = tools.iter().any(|t| t.state == ToolState::Failed);
    let summary = if running > 0 {
        tools.iter().rev().find(|t| t.state == ToolState::Running).map(|t| format!("{}…", t.label())).unwrap_or_default()
    } else {
        format!("Used {} tool{}", tools.len(), if tools.len() == 1 { "" } else { "s" })
    };
    let lead: El = if running > 0 { spinner(app.phase_t) } else { icon(icons::WRENCH).size(14).color(c::TEXT_3).into() };
    let head = button(
        row![lead, text(summary).size(13).color(if failed { c::RED } else { c::TEXT_2 }), icon(if expanded { icons::CHEVRON_DOWN } else { icons::CHEVRON_RIGHT }).size(13).color(c::TEXT_3)]
            .spacing(8)
            .align_y(Alignment::Center),
    )
    .padding([4, 8])
    .style(theme::btn_ghost)
    .on_press(Msg::ToggleActivity(i));
    let mut col = column![head].spacing(2);
    if expanded || running > 0 {
        let mut list = column![].spacing(6);
        for tcall in tools {
            let (ic, colr) = match tcall.state {
                ToolState::Running => (icons::LOADER, c::ACCENT),
                ToolState::Done => (tcall.icon(), c::TEXT_3),
                ToolState::Failed => (icons::CIRCLE_ALERT, c::RED),
            };
            let mut r = row![icon(ic).size(13).color(colr), text(tcall.label()).size(13).color(c::TEXT_2)].spacing(8).align_y(Alignment::Center);
            if !tcall.context.is_empty() {
                r = r.push(text(one_line(&tcall.context, 48)).size(12).font(theme::MONO).color(c::TEXT_3));
            }
            r = r.push(Space::new().width(Fill));
            if !tcall.summary.is_empty() {
                r = r.push(faint(one_line(&tcall.summary, 40)));
            } else if let Some(p) = tcall.percent {
                r = r.push(faint(format!("{p}%")));
            }
            list = list.push(r);
        }
        col = col.push(container(list).padding(Padding { top: 6.0, bottom: 8.0, left: 14.0, right: 12.0 }).style(|_: &iced::Theme| container::Style {
            border: iced::Border { color: c::BORDER, width: 0.0, radius: 0.0.into() },
            ..Default::default()
        }));
    }
    container(col).padding(Padding { left: 36.0, ..Default::default() }).into()
}

// ---------------------------------------------------------------- details pane

fn details(app: &App) -> El<'_> {
    let Some(bot) = app.selected_bot() else { return Space::new().into() };
    let tabs = row![
        button(text("Computer").size(13)).padding([6, 12]).style(theme::btn_tab(app.details_tab == DetailsTab::Computer)).on_press(Msg::DetailsTab(DetailsTab::Computer)),
        button(text("About").size(13)).padding([6, 12]).style(theme::btn_tab(app.details_tab == DetailsTab::About)).on_press(Msg::DetailsTab(DetailsTab::About)),
        Space::new().width(Fill),
        icon_btn(icons::X, Msg::ToggleDetails, "Close (Esc)"),
    ]
    .spacing(4)
    .align_y(Alignment::Center);
    let profile = column![avatar(bot.color, app.status_of(&bot.name), app.phase_t, 56.0, false), heading(bot.title.as_str(), 18.0), muted(bot.description.as_str())].spacing(8);
    let body: El = match app.details_tab {
        DetailsTab::Computer => computer_tab(app, bot),
        DetailsTab::About => column![
            kv("Profile", bot.name.clone()),
            kv("Model", if bot.model.is_empty() { "Default".into() } else { bot.model.clone() }),
            kv("Bot Chat", "One permanent chat".into()),
            faint("Edit the role, model and skills in Hermes Agent (profile settings)."),
        ]
        .spacing(10)
        .into(),
    };
    container(column![tabs, profile, container(Space::new()).width(Fill).height(1).style(theme::solid(c::BORDER, 0.0)), body].spacing(16))
        .padding(16)
        .width(DETAILS_W)
        .height(Fill)
        .style(theme::panel_right)
        .into()
}

fn kv<'a>(k: &'a str, v: String) -> El<'a> {
    row![muted(k).width(90), t(v)].spacing(8).into()
}

fn computer_tab<'a>(app: &'a App, bot: &'a Bot) -> El<'a> {
    let st = app.screen.status.as_ref().filter(|s| s.profile == bot.name);
    let running = st.is_some_and(|s| s.running);
    let human = st.is_some_and(|s| s.lease.human_holds());
    let state_text = match (st, running) {
        (None, _) => "Checking…",
        (Some(s), _) if !s.supported || !s.installed => "Not set up",
        (_, false) => "Asleep",
        _ if human => "You are in control",
        _ => "Working",
    };
    let preview: El = if running {
        let inner: El = match &app.screen.frame {
            Some(f) if app.screen.profile.as_deref() == Some(&bot.name) => image(f.clone()).content_fit(ContentFit::Contain).width(Fill).height(Fill).into(),
            _ => center(column![icon(icons::MONITOR).size(28).color(c::TEXT_3), faint("Live view")].spacing(6).align_x(Alignment::Center)).into(),
        };
        button(container(inner).width(Fill).height(205).style(theme::screen_frame(false))).padding(0).style(theme::btn_ghost).on_press(Msg::OpenScreen { take_over: false }).into()
    } else {
        container(
            center(
                column![
                    icon(icons::MONITOR).size(30).color(c::TEXT_3),
                    t(format!("{}'s computer is asleep", bot.title)),
                    if app.screen.connecting { Element::from(row![spinner(app.phase_t), muted("Waking up…")].spacing(8)) } else { secondary("Wake up", Some(Msg::WakeScreen)).into() }
                ]
                .spacing(10)
                .align_x(Alignment::Center),
            ),
        )
        .width(Fill)
        .height(205)
        .style(theme::screen_frame(false))
        .into()
    };
    let mut col = column![
        row![text("Computer").size(14).font(theme::SEMIBOLD).color(c::TEXT), Space::new().width(Fill), container(text(state_text).size(11).font(theme::SEMIBOLD)).padding([2, 8]).style(theme::pill(if running { c::ACCENT } else { c::TEXT_3 }))].align_y(Alignment::Center),
        preview,
    ]
    .spacing(10);
    if running {
        col = col.push(
            row![
                button(row![icon(icons::MAXIMIZE).color(c::TEXT), text("Open").size(13)].spacing(6).align_y(Alignment::Center)).padding([8, 12]).style(theme::btn_secondary).on_press(Msg::OpenScreen { take_over: false }),
                button(row![icon(icons::HAND).color(c::ON_ACCENT), text("Take over").size(13).font(theme::SEMIBOLD)].spacing(6).align_y(Alignment::Center)).padding([8, 12]).style(theme::btn_primary).on_press(Msg::OpenScreen { take_over: true }),
            ]
            .spacing(8),
        );
        col = col.push(faint("Take over to sign in to a website on the Bot's screen. Hand back when you are done."));
    }
    if let Some(e) = &app.screen.error {
        col = col.push(container(row![icon(icons::CIRCLE_ALERT).color(c::RED), text(e.as_str()).size(12).color(c::TEXT)].spacing(8)).padding([8, 10]).width(Fill).style(theme::banner(c::RED)));
    }
    col.into()
}

// ---------------------------------------------------------------- full screen

fn screen_full(app: &App) -> El<'_> {
    let bot = app.screen.profile.as_ref().and_then(|p| app.bots.iter().find(|b| &b.name == p));
    let title = bot.map(|b| b.title.clone()).unwrap_or_default();
    let color = bot.map(|b| b.color).unwrap_or(c::ACCENT);
    let control = app.screen.mode == ScreenMode::Control;
    let state = if control { "You're in control" } else { "Watching" };
    let top = row![
        icon_btn(icons::ARROW_LEFT, Msg::CloseScreen, "Back to chat (Esc)"),
        avatar(color, BotStatus::Idle, 0.0, 26.0, false),
        text(format!("{title}'s computer")).size(15).font(theme::SEMIBOLD).color(c::TEXT),
        container(text(state).size(11).font(theme::SEMIBOLD)).padding([2, 8]).style(theme::pill(if control { c::ACCENT } else { c::TEXT_2 })),
        Space::new().width(Fill),
        button(row![icon(icons::MINIMIZE).color(c::TEXT_2), text("Exit full screen").size(13)].spacing(6).align_y(Alignment::Center)).padding([6, 10]).style(theme::btn_ghost).on_press(Msg::CloseScreen),
    ]
    .spacing(10)
    .align_y(Alignment::Center)
    .padding([8, 14]);

    let screen_inner: El = match (&app.screen.frame, &app.screen.error) {
        (_, Some(e)) if app.screen.frame.is_none() => center(
            column![icon(icons::CIRCLE_ALERT).size(30).color(c::RED), t(format!("Couldn't reach {title}'s computer")), muted(e.as_str()), secondary("Retry", Some(Msg::OpenScreen { take_over: false }))]
                .spacing(10)
                .align_x(Alignment::Center),
        )
        .into(),
        (Some(f), _) => {
            let img = image(f.clone()).content_fit(ContentFit::Contain).filter_method(image::FilterMethod::Linear).width(Fill).height(Fill);
            let layer = iced::widget::canvas(crate::screen_input::ScreenInput { fb: app.screen.frame_size, enabled: control }).width(Fill).height(Fill);
            stack![img, layer].into()

        }
        (None, _) => center(column![spinner(app.phase_t), muted(if app.online() { "Connecting to the Bot's screen…" } else { "Reconnecting…" })].spacing(10).align_x(Alignment::Center)).into(),
    };
    let mut layers: Vec<El> = vec![container(screen_inner).width(Fill).height(Fill).padding(2).style(theme::screen_frame(control)).into()];
    if app.screen.connecting && app.screen.frame.is_some() {
        layers.push(container(container(row![spinner(app.phase_t), muted("Reconnecting…")].spacing(8)).padding([6, 12]).style(theme::floating_pill)).width(Fill).padding(12).align_x(Alignment::Center).into());
    }
    let pill: El = if control {
        container(
            row![
                container(Space::new()).width(8).height(8).style(theme::solid(c::ACCENT, 4.0)),
                t("You're in control"),
                faint(format!("{title} is paused")),
                button(row![icon(icons::CHECK).color(c::ON_ACCENT), text("I'm done").size(14).font(theme::SEMIBOLD)].spacing(6).align_y(Alignment::Center)).padding([7, 14]).style(theme::btn_primary).on_press(Msg::HandBack),
            ]
            .spacing(12)
            .align_y(Alignment::Center),
        )
        .padding(Padding { top: 6.0, bottom: 6.0, left: 16.0, right: 6.0 })
        .style(theme::floating_pill)
        .into()
    } else {
        container(
            row![
                icon(icons::EYE).color(c::TEXT_2),
                t("Watching"),
                faint("Your input does not reach the Bot"),
                button(row![icon(icons::HAND).color(c::ON_ACCENT), text("Take control").size(14).font(theme::SEMIBOLD)].spacing(6).align_y(Alignment::Center))
                    .padding([7, 14])
                    .style(theme::btn_primary)
                    .on_press_maybe(app.screen.viewer_id.is_some().then_some(Msg::TakeControl)),
            ]
            .spacing(12)
            .align_y(Alignment::Center),
        )
        .padding(Padding { top: 6.0, bottom: 6.0, left: 16.0, right: 6.0 })
        .style(theme::floating_pill)
        .into()
    };
    layers.push(container(pill).width(Fill).height(Fill).align_x(Alignment::Center).align_y(Alignment::End).padding(20).into());
    let mut col = column![top];
    if control {
        col = col.push(
            container(row![icon(icons::HAND).color(c::ON_ACCENT), text(format!("You're in control. {title} is paused. Keys and clicks go to its screen.")).size(13).font(theme::MEDIUM).color(c::ON_ACCENT)].spacing(8).align_y(Alignment::Center))
                .padding([7, 14])
                .width(Fill)
                .style(theme::solid(c::ACCENT, 0.0)),
        );
    }
    col = col.push(container(stack(layers)).padding(12).width(Fill).height(Fill));
    container(col).width(Fill).height(Fill).style(theme::pane).into()
}

// ---------------------------------------------------------------- settings

fn settings_view(app: &App, pane: SettingsPane) -> El<'_> {
    let nav_btn = |label: &'static str, ic: char, p: SettingsPane| -> El<'_> {
        button(row![icon(ic).size(15).color(c::TEXT_2), text(label).size(14)].spacing(10).align_y(Alignment::Center))
            .width(Fill)
            .padding([8, 10])
            .style(theme::btn_tab(pane == p))
            .on_press(Msg::OpenSettings(p))
            .into()
    };
    let nav = column![
        heading("Settings", 18.0),
        Space::new().height(8),
        nav_btn("General", icons::SETTINGS, SettingsPane::General),
        nav_btn("Connection", icons::GLOBE, SettingsPane::Connection),
        nav_btn("About", icons::SPARKLES, SettingsPane::About),
    ]
    .spacing(4)
    .width(180);
    let toggle = |label: &'static str, hint: &'static str, on: bool, msg: Msg| -> El<'static> {
        row![
            column![t(label), faint(hint)].spacing(2).width(Fill),
            button(container(Space::new()).width(16).height(16).style(theme::solid(if on { c::ON_ACCENT } else { c::TEXT_2 }, 8.0)))
                .padding(Padding { top: 3.0, bottom: 3.0, left: if on { 21.0 } else { 3.0 }, right: if on { 3.0 } else { 21.0 } })
                .style(move |th, s| if on { theme::btn_primary(th, s) } else { theme::btn_secondary(th, s) })
                .on_press(msg)
        ]
        .spacing(12)
        .align_y(Alignment::Center)
        .into()
    };
    let me = app.me.clone().unwrap_or_default();
    let content: El = match pane {
        SettingsPane::General => column![
            heading("General", 16.0),
            row![
                column![t(if me.display_name.is_empty() { "Signed in".to_string() } else { me.display_name.clone() }), faint(format!("via {}", if me.provider.is_empty() { "gateway sign-in" } else { me.provider.as_str() }))].spacing(2).width(Fill),
                button(row![icon(icons::LOG_OUT).color(c::RED), text("Sign out").size(13)].spacing(6).align_y(Alignment::Center)).padding([8, 12]).style(theme::btn_danger).on_press(Msg::SignOut)
            ]
            .align_y(Alignment::Center),
            toggle("Hermes wallpaper", "Show the Hermes background art behind the app.", app.settings.show_wallpaper, Msg::ToggleWallpaper),
            toggle("Reduce motion", "Stop avatar and spinner animation.", app.settings.reduce_motion, Msg::ToggleReduceMotion),
        ]
        .spacing(18)
        .into(),
        SettingsPane::Connection => {
            let host = app.session.as_ref().map(|s| s.gateway.to_string()).unwrap_or_default();
            let state = match &app.conn {
                ConnState::Online => "Connected".to_string(),
                ConnState::Connecting => "Connecting…".to_string(),
                ConnState::Offline { reason, .. } => format!("Offline: {reason}"),
            };
            column![
                heading("Connection", 16.0),
                kv("Gateway", host),
                kv("State", state),
                kv("Token store", if crate::store::keyring_available() { "System keyring (Secret Service)".into() } else { "Memory only (no keyring found)".into() }),
                faint("To use another gateway, sign out and enter the new address."),
            ]
            .spacing(14)
            .into()
        }
        SettingsPane::About => column![
            heading("About", 16.0),
            kv("Version", env!("CARGO_PKG_VERSION").into()),
            kv("Toolkit", "iced (native Rust, no webview)".into()),
            muted("Hermes Bots is an independent, open-source client for the Hermes Agent API. It is not made by Nous Research."),
            faint("Fonts: Inter, JetBrains Mono (OFL), Lucide icons (ISC). Wallpaper and colours from Hermes Agent (MIT)."),
        ]
        .spacing(14)
        .into(),
    };
    container(row![nav, container(Space::new()).width(1).height(Fill).style(theme::solid(c::BORDER, 0.0)), container(scrollable(content).style(theme::scroll)).padding(Padding { left: 8.0, ..Default::default() }).width(Fill)].spacing(16))
        .padding(22)
        .width(720)
        .height(480)
        .style(theme::dialog)
        .into()
}

// ---------------------------------------------------------------- create Bot

fn create_view(f: &CreateForm) -> El<'_> {
    let color = theme::parse_hex(Some(SWATCHES[f.color]));
    let mut swatches = row![].spacing(8);
    for (i, s) in SWATCHES.iter().enumerate() {
        let col = theme::parse_hex(Some(s));
        let sel = i == f.color;
        swatches = swatches.push(
            button(container(if sel { Element::from(icon(icons::CHECK).size(13).color(c::ON_ACCENT)) } else { Space::new().into() }).center(Length::Fixed(26.0)).style(theme::solid(col, 13.0)))
                .padding(2)
                .style(move |_, _| button::Style {
                    border: iced::Border { color: if sel { c::TEXT } else { Color::TRANSPARENT }, width: 2.0, radius: 16.0.into() },
                    ..Default::default()
                })
                .on_press(Msg::CreateColor(i)),
        );
    }
    let mut col = column![
        row![avatar(color, BotStatus::Idle, 0.0, 48.0, false), column![heading("Create a new Bot", 18.0), muted("A Bot has one job, its own memory and its own computer.")].spacing(2)].spacing(14).align_y(Alignment::Center),
        text("Name").size(13).font(theme::MEDIUM).color(c::TEXT_2),
        text_input("For example: Research Buddy", &f.title).id("create-name").on_input(Msg::CreateTitle).on_submit(Msg::CreateSubmit).padding([9, 12]).size(14).style(theme::input),
        row![faint("Profile id:"), text(if f.name.is_empty() { "—".to_string() } else { f.name.clone() }).size(12).font(theme::MONO).color(c::TEXT_2)].spacing(6),
        text("Job").size(13).font(theme::MEDIUM).color(c::TEXT_2),
        text_input("What should this Bot do?", &f.description).on_input(Msg::CreateDesc).on_submit(Msg::CreateSubmit).padding([9, 12]).size(14).style(theme::input),
        text("Colour").size(13).font(theme::MEDIUM).color(c::TEXT_2),
        swatches,
    ]
    .spacing(8);
    if let Some(e) = &f.error {
        col = col.push(container(row![icon(icons::CIRCLE_ALERT).color(c::RED), text(e.as_str()).size(13)].spacing(8)).padding([8, 12]).width(Fill).style(theme::banner(c::RED)));
    }
    col = col.push(Space::new().height(6)).push(
        row![
            Space::new().width(Fill),
            secondary("Cancel", Some(Msg::CreateCancel)),
            primary(if f.busy { "Creating…" } else { "Create" }, (!f.busy && !f.title.trim().is_empty()).then_some(Msg::CreateSubmit))
        ]
        .spacing(8),
    );
    container(col).padding(24).width(480).height(Shrink).style(theme::dialog).into()
}
