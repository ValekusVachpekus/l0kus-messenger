//! Рендер интерфейса средствами ratatui (сдержанная тема).

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Clear, List, ListItem, Paragraph, Wrap};

use super::app::{App, Field, Screen, Who};
use super::files::{EntryKind, FileBrowser};
use super::theme;

pub fn draw(frame: &mut Frame, app: &App) {
    match app.screen {
        Screen::Setup => draw_setup(frame, app),
        Screen::Chat => {
            draw_chat(frame, app);
            if let Some(browser) = &app.file_browser {
                draw_file_browser(frame, browser);
            } else if app.show_help {
                draw_help(frame);
            }
        }
    }
}

// --- экран чата --------------------------------------------------------------

fn draw_chat(frame: &mut Frame, app: &App) {
    let root = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(18), Constraint::Min(20)])
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
                (theme::ICON_ONLINE, theme::ONLINE)
            } else {
                (theme::ICON_OFFLINE, theme::OFFLINE)
            };
            let name_style = if selected {
                Style::default().fg(theme::TEXT).bg(theme::SEL_BG)
            } else if p.online {
                Style::default().fg(theme::TEXT)
            } else {
                Style::default().fg(theme::MUTED)
            };
            let mut spans = vec![
                Span::styled(format!(" {dot} "), Style::default().fg(dot_color)),
                Span::styled(p.nick.clone(), name_style),
            ];
            if p.verified {
                spans.push(Span::styled(
                    format!(" {}", theme::ICON_VERIFIED),
                    Style::default().fg(theme::ONLINE),
                ));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();

    let list = List::new(items).block(theme::panel(&format!("пиры ({})", app.peers.len())));
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
    let title = format!("{} · fp {}", app.me_nick, app.me_fingerprint);
    let line = match app.selected_peer() {
        Some(p) => {
            let trust = if p.verified {
                Span::styled(
                    format!("доверен {}", theme::ICON_VERIFIED),
                    Style::default().fg(theme::ONLINE),
                )
            } else {
                Span::styled("не сверен", Style::default().fg(theme::WARN))
            };
            Line::from(vec![
                Span::styled(
                    p.nick.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled("  ", Style::default()),
                trust,
                Span::styled(
                    format!("  fp {}", p.fingerprint),
                    Style::default().fg(theme::MUTED),
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
                Who::Me => ("> ", Style::default().fg(theme::ACCENT)),
                Who::Them => ("< ", Style::default().fg(theme::TEXT)),
                Who::System => ("* ", Style::default().fg(theme::MUTED)),
            };
            lines.push(Line::from(vec![
                Span::styled(format!("{} ", line.time), Style::default().fg(theme::MUTED)),
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
    let title = if scrolled { "чат [↑]" } else { "чат" };
    let p = Paragraph::new(window)
        .block(theme::panel(title))
        .wrap(Wrap { trim: false });
    frame.render_widget(p, area);
}

fn draw_input(frame: &mut Frame, app: &App, area: Rect) {
    let p = Paragraph::new(Span::styled(
        app.input.as_str(),
        Style::default().fg(theme::TEXT),
    ))
    .block(theme::panel_active("ввод · Enter — отправить"));
    frame.render_widget(p, area);
    // Курсор в конце ввода.
    let x = area.x + 1 + app.input.chars().count() as u16;
    let y = area.y + 1;
    frame.set_cursor_position((x.min(area.x + area.width - 2), y));
}

fn draw_status(frame: &mut Frame, app: &App, area: Rect) {
    let listen = app.listen_addrs.first().map(String::as_str).unwrap_or("—");
    let hints = "Tab пир · ^V сверить · ^F файл · PgUp/Dn скролл · ? помощь";
    let mut spans = vec![
        Span::styled(hints, Style::default().fg(theme::MUTED)),
        Span::styled("  ", Style::default()),
        Span::styled(app.status.clone(), Style::default().fg(theme::TEXT)),
    ];
    if !app.listen_addrs.is_empty() {
        spans.push(Span::styled(
            format!("  {listen}"),
            Style::default().fg(theme::MUTED),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

// --- стартовый экран ---------------------------------------------------------

fn draw_setup(frame: &mut Frame, app: &App) {
    let area = centered_rect(54, 16, frame.area());
    frame.render_widget(Clear, area);

    let f = &app.setup;
    let mut lines: Vec<Line> = vec![
        Line::from(Span::styled(
            "l0kus",
            Style::default()
                .fg(theme::ACCENT)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "защищённый p2p-мессенджер · olm e2e",
            Style::default().fg(theme::MUTED),
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
            format!("{} {err}", theme::ICON_WARN),
            Style::default().fg(theme::WARN),
        )));
    }
    lines.push(Line::from(Span::styled(
        "Tab — поле · Space — галка · Enter — запуск · Esc — выход",
        Style::default().fg(theme::MUTED),
    )));

    let p = Paragraph::new(lines)
        .alignment(Alignment::Center)
        .block(theme::panel_active("настройка"));
    frame.render_widget(p, area);
}

/// Строка текстового поля формы. Активное — с маркером и кареткой.
fn field_line<'a>(label: &'a str, value: &'a str, active: bool, placeholder: &'a str) -> Line<'a> {
    let marker = if active { "> " } else { "  " };
    let (shown, style) = if value.is_empty() {
        (placeholder.to_string(), Style::default().fg(theme::OFFLINE))
    } else {
        (value.to_string(), Style::default().fg(theme::TEXT))
    };
    let caret = if active { "_" } else { "" };
    let label_style = if active {
        Style::default().fg(theme::ACCENT)
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
    let marker = if active { "> " } else { "  " };
    let box_ = if checked { "[x]" } else { "[ ]" };
    let style = if active {
        Style::default().fg(theme::ACCENT)
    } else {
        Style::default().fg(theme::MUTED)
    };
    Line::from(vec![
        Span::styled(marker, Style::default().fg(theme::ACCENT)),
        Span::styled(format!("{box_} Запоминать настройки"), style),
    ])
}

// --- файловый браузер --------------------------------------------------------

fn draw_file_browser(frame: &mut Frame, browser: &FileBrowser) {
    let area = centered_rect(72, 22, frame.area());
    frame.render_widget(Clear, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .split(area);

    let inner_h = rows[0].height.saturating_sub(2) as usize;

    let mut items: Vec<ListItem> = Vec::new();
    if let Some(err) = &browser.error {
        items.push(ListItem::new(Line::from(Span::styled(
            format!("⚠ {err}"),
            Style::default().fg(theme::WARN),
        ))));
    } else if browser.entries.is_empty() {
        items.push(ListItem::new(Line::from(Span::styled(
            format!("{} (пусто)", theme::ICON_WARN),
            Style::default().fg(theme::MUTED),
        ))));
    }

    // Окно прокрутки вокруг выбранной записи.
    let total = browser.entries.len();
    let cap = inner_h.max(1);
    let start = browser
        .selected
        .saturating_sub(cap / 2)
        .min(total.saturating_sub(cap));
    for (i, e) in browser.entries.iter().enumerate().skip(start).take(cap) {
        let selected = i == browser.selected;
        let (color, icon) = match e.kind {
            EntryKind::Dir => (theme::DIR, theme::ICON_DIR),
            EntryKind::Exec => (theme::EXEC, theme::ICON_EXEC),
            EntryKind::Link => (theme::LINK, theme::ICON_LINK),
            EntryKind::File => (theme::TEXT, theme::ICON_FILE_PLAIN),
        };
        let mut style = Style::default().fg(color);
        if e.kind == EntryKind::Dir {
            style = style.add_modifier(Modifier::BOLD);
        }
        if selected {
            style = style.bg(theme::SEL_BG);
        }
        let prefix = if selected { "> " } else { "  " };
        items.push(ListItem::new(Line::from(vec![
            Span::styled(prefix, Style::default().fg(theme::ACCENT)),
            Span::styled(format!("{icon}  {}", e.name), style),
        ])));
    }

    let title = format!("выбор файла · {}", browser.cwd.display());
    let list = List::new(items).block(theme::panel_active(&title));
    frame.render_widget(list, rows[0]);

    let hint = Paragraph::new(Line::from(Span::styled(
        " ↑↓ — выбор · Enter — войти/отправить · Backspace — назад · Esc — отмена",
        Style::default().fg(theme::MUTED),
    )));
    frame.render_widget(hint, rows[1]);
}

// --- оверлей помощи ----------------------------------------------------------

fn draw_help(frame: &mut Frame) {
    let area = centered_rect(54, 15, frame.area());
    frame.render_widget(Clear, area);

    let rows = [
        ("Enter", "отправить сообщение выбранному пиру"),
        ("Tab / ↑ ↓", "переключение между пирами"),
        ("Ctrl+V", "пометить пира сверенным (fingerprint)"),
        ("Ctrl+F", "выбрать файл на отправку (браузер)"),
        ("Ctrl+R", "пересканировать LAN (поиск пиров)"),
        ("/dial <адрес>", "подключиться к пиру вручную"),
        ("/file <путь>", "отправить файл по пути"),
        ("PgUp / PgDn", "прокрутка истории чата"),
        ("? / F1", "показать/скрыть эту справку"),
        ("Esc / Ctrl+C", "выход"),
    ];
    let mut lines: Vec<Line> = Vec::new();
    for (key, desc) in rows {
        lines.push(Line::from(vec![
            Span::styled(format!("{key:<14}"), Style::default().fg(theme::ACCENT)),
            Span::styled(desc, Style::default().fg(theme::TEXT)),
        ]));
    }
    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled(
        "любая клавиша — закрыть",
        Style::default().fg(theme::MUTED),
    )));

    frame.render_widget(Paragraph::new(lines).block(theme::panel_active("помощь")), area);
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
