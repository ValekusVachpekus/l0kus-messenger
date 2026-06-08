//! Рендер интерфейса средствами ratatui (тема «неоновый циан»).

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, List, ListItem, Paragraph, Wrap};

use super::app::{App, Field, Screen, Who};
use super::theme;

pub fn draw(frame: &mut Frame, app: &App) {
    match app.screen {
        Screen::Setup => draw_setup(frame, app),
        Screen::Chat => {
            draw_chat(frame, app);
            if app.show_help {
                draw_help(frame);
            }
        }
    }
}

// --- экран чата --------------------------------------------------------------

fn draw_chat(frame: &mut Frame, app: &App) {
    let root = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(30), Constraint::Min(20)])
        .split(frame.area());

    draw_peers(frame, app, root[0]);
    draw_right(frame, app, root[1]);
}

fn draw_peers(frame: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .peers
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let selected = i == app.selected;
            let (dot, dot_color) = if p.online {
                ("●", theme::ONLINE)
            } else {
                ("○", theme::OFFLINE)
            };
            let name_style = if selected {
                Style::default()
                    .fg(theme::TEXT)
                    .bg(theme::SEL_BG)
                    .add_modifier(Modifier::BOLD)
            } else if p.online {
                Style::default().fg(theme::TEXT)
            } else {
                Style::default().fg(theme::MUTED)
            };
            let mut spans = vec![
                Span::raw(if selected { "▌" } else { " " }),
                Span::styled(format!("{dot} "), Style::default().fg(dot_color)),
                Span::styled(p.nick.clone(), name_style),
            ];
            if p.verified {
                spans.push(Span::styled(" ✓", Style::default().fg(theme::ONLINE)));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();

    let list = List::new(items).block(theme::panel(&format!("PEERS ({})", app.peers.len())));
    frame.render_widget(list, area);
}

fn draw_right(frame: &mut Frame, app: &App, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(3),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(area);

    draw_header(frame, app, rows[0]);
    draw_messages(frame, app, rows[1]);
    draw_input(frame, app, rows[2]);
    draw_status(frame, app, rows[3]);
}

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let title = format!("L0KUS // {} · fp {}", app.me_nick, app.me_fingerprint);
    let line = match app.selected_peer() {
        Some(p) => {
            let trust = if p.verified {
                Span::styled(" доверен ✓ ", Style::default().fg(theme::ONLINE))
            } else {
                Span::styled(" не сверен ", Style::default().fg(theme::WARN))
            };
            Line::from(vec![
                Span::styled(
                    p.nick.clone(),
                    Style::default()
                        .fg(theme::ACCENT)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("│", Style::default().fg(theme::ACCENT_DIM)),
                trust,
                Span::styled(
                    format!("fp {}", p.fingerprint),
                    Style::default().fg(theme::MAGENTA),
                ),
            ])
        }
        None => Line::from(Span::styled(
            "нет выбранного пира",
            Style::default().fg(theme::MUTED),
        )),
    };
    let p = Paragraph::new(line)
        .block(theme::panel(&title))
        .wrap(Wrap { trim: true });
    frame.render_widget(p, area);
}

fn draw_messages(frame: &mut Frame, app: &App, area: Rect) {
    let mut lines: Vec<Line> = Vec::new();
    if let Some(peer) = app.selected_peer() {
        for line in &peer.messages {
            let (prefix, style) = match line.who {
                Who::Me => ("→ ", Style::default().fg(theme::ACCENT)),
                Who::Them => ("← ", Style::default().fg(theme::MAGENTA)),
                Who::System => ("· ", Style::default().fg(theme::MUTED)),
            };
            lines.push(Line::from(vec![
                Span::styled(format!("[{}] ", line.time), Style::default().fg(theme::MUTED)),
                Span::styled(prefix, style),
                Span::styled(line.text.clone(), Style::default().fg(theme::TEXT)),
            ]));
        }
    } else {
        lines.push(Line::from(Span::styled(
            "Ждём пиров. Они появятся автоматически (mDNS) или после /dial <адрес>.",
            Style::default().fg(theme::MUTED),
        )));
    }

    // Окно с учётом прокрутки: 0 — низ, app.scroll строк вверх.
    let cap = area.height.saturating_sub(2) as usize;
    let total = lines.len();
    let max_start = total.saturating_sub(cap.max(1));
    let start = max_start.saturating_sub(app.scroll);
    let window: Vec<Line> = lines.into_iter().skip(start).collect();

    let scrolled = app.scroll > 0 && max_start > 0;
    let title = if scrolled { "CHAT ↑" } else { "CHAT" };
    let p = Paragraph::new(window)
        .block(theme::panel(title))
        .wrap(Wrap { trim: false });
    frame.render_widget(p, area);
}

fn draw_input(frame: &mut Frame, app: &App, area: Rect) {
    let prompt = "▌ ";
    let line = Line::from(vec![
        Span::styled(prompt, Style::default().fg(theme::ACCENT)),
        Span::styled(app.input.as_str(), Style::default().fg(theme::TEXT)),
    ]);
    let p = Paragraph::new(line).block(theme::panel_active("INPUT · Enter — отправить"));
    frame.render_widget(p, area);
    // Курсор после приглашения и текста.
    let x = area.x + 1 + prompt.chars().count() as u16 + app.input.chars().count() as u16;
    let y = area.y + 1;
    frame.set_cursor_position((x.min(area.x + area.width - 2), y));
}

fn draw_status(frame: &mut Frame, app: &App, area: Rect) {
    let listen = app.listen_addrs.first().map(String::as_str).unwrap_or("—");
    let mut spans = vec![
        chip("Tab"),
        Span::raw(" пир  "),
        chip("^V"),
        Span::raw(" сверить  "),
        chip("PgUp/Dn"),
        Span::raw(" скролл  "),
        chip("?"),
        Span::raw(" помощь  "),
        Span::styled("│ ", Style::default().fg(theme::ACCENT_DIM)),
        Span::styled(app.status.clone(), Style::default().fg(theme::MUTED)),
    ];
    if !app.listen_addrs.is_empty() {
        spans.push(Span::styled(
            format!("  ◇ {listen}"),
            Style::default().fg(theme::ACCENT_DIM),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

/// «Чип» хоткея — акцентная подпись клавиши.
fn chip(key: &str) -> Span<'_> {
    Span::styled(
        key,
        Style::default()
            .fg(theme::ACCENT)
            .add_modifier(Modifier::BOLD),
    )
}

// --- стартовый экран ---------------------------------------------------------

fn draw_setup(frame: &mut Frame, app: &App) {
    let area = centered_rect(56, 17, frame.area());
    frame.render_widget(Clear, area);

    let f = &app.setup;
    let mut lines: Vec<Line> = vec![
        Line::from(Span::styled(
            "L 0 K U S",
            Style::default()
                .fg(theme::ACCENT)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "защищённый p2p-мессенджер · olm e2e",
            Style::default().fg(theme::MAGENTA),
        )),
        Line::from(Span::styled(
            format!("fp {}", app.me_fingerprint),
            Style::default().fg(theme::MUTED),
        )),
        Line::raw(""),
        field_line("Ник  ", &f.nick, f.current() == Field::Nick, "имя пользователя ОС"),
        field_line("Порт ", &f.port, f.current() == Field::Port, "случайный"),
        field_line("Адрес", &f.dial, f.current() == Field::Dial, "необязательно"),
        checkbox_line(f.remember, f.current() == Field::Remember),
        Line::raw(""),
    ];
    if let Some(err) = &f.error {
        lines.push(Line::from(Span::styled(
            format!("⚠ {err}"),
            Style::default().fg(theme::WARN),
        )));
    }
    lines.push(Line::from(vec![
        chip("Tab"),
        Span::styled(" поле  ", Style::default().fg(theme::MUTED)),
        chip("Space"),
        Span::styled(" галка  ", Style::default().fg(theme::MUTED)),
        chip("Enter"),
        Span::styled(" запуск  ", Style::default().fg(theme::MUTED)),
        chip("Esc"),
        Span::styled(" выход", Style::default().fg(theme::MUTED)),
    ]));

    let p = Paragraph::new(lines)
        .alignment(Alignment::Center)
        .block(theme::panel_active("НАСТРОЙКА"));
    frame.render_widget(p, area);
}

/// Строка текстового поля формы. Активное — с акцентным маркером и кареткой.
fn field_line<'a>(label: &'a str, value: &'a str, active: bool, placeholder: &'a str) -> Line<'a> {
    let marker = if active { "▸ " } else { "  " };
    let (shown, style) = if value.is_empty() {
        (placeholder.to_string(), Style::default().fg(theme::OFFLINE))
    } else {
        (value.to_string(), Style::default().fg(theme::TEXT))
    };
    let caret = if active { "▏" } else { "" };
    let label_style = if active {
        Style::default().fg(theme::ACCENT).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme::MUTED)
    };
    Line::from(vec![
        Span::styled(marker, Style::default().fg(theme::ACCENT)),
        Span::styled(format!("{label}  "), label_style),
        Span::styled(format!("[ {shown}{caret} ]"), style),
    ])
}

fn checkbox_line(checked: bool, active: bool) -> Line<'static> {
    let marker = if active { "▸ " } else { "  " };
    let box_ = if checked { "[x]" } else { "[ ]" };
    let style = if active {
        Style::default().fg(theme::ACCENT).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme::MUTED)
    };
    Line::from(vec![
        Span::styled(marker, Style::default().fg(theme::ACCENT)),
        Span::styled(format!("{box_} Запоминать настройки"), style),
    ])
}

// --- оверлей помощи ----------------------------------------------------------

fn draw_help(frame: &mut Frame) {
    let area = centered_rect(54, 14, frame.area());
    frame.render_widget(Clear, area);

    let rows = [
        ("Enter", "отправить сообщение выбранному пиру"),
        ("Tab / ↑ ↓", "переключение между пирами"),
        ("Ctrl+V", "пометить пира сверенным (fingerprint)"),
        ("/dial <адрес>", "подключиться к пиру вручную"),
        ("/file <путь>", "отправить файл выбранному пиру"),
        ("PgUp / PgDn", "прокрутка истории чата"),
        ("? / F1", "показать/скрыть эту справку"),
        ("Esc / Ctrl+C", "выход"),
    ];
    let mut lines: Vec<Line> = Vec::new();
    for (key, desc) in rows {
        lines.push(Line::from(vec![
            Span::styled(
                format!("{key:<14}"),
                Style::default()
                    .fg(theme::ACCENT)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(desc, Style::default().fg(theme::TEXT)),
        ]));
    }
    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled(
        "любая клавиша — закрыть",
        Style::default().fg(theme::MUTED),
    )));

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme::ACCENT))
        .title_style(Style::default().fg(theme::ACCENT).add_modifier(Modifier::BOLD))
        .title(" ◈ ПОМОЩЬ ");
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

/// Прямоугольник заданного размера по центру `area` (с ограничением размером).
fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect {
        x,
        y,
        width,
        height,
    }
}
