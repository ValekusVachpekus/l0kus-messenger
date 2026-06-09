//! TUI: цикл рендера и ввода (ratatui + crossterm).

pub mod app;
pub mod events;
pub mod files;
pub mod theme;
pub mod view;

use anyhow::Result;
use crossterm::event::{Event, EventStream, KeyEventKind};
use futures::StreamExt;
use tokio::sync::mpsc;

use crate::message::{AppEvent, UiCommand};
use app::App;

/// Начальные параметры запуска TUI.
pub struct StartupConfig {
    /// Показывать ли стартовый экран настройки (true) или сразу чат (false).
    pub interactive: bool,
    /// Предзаполненный ник.
    pub nick: String,
    /// Предзаполненный порт (0 — поле пустое/случайный порт).
    pub port: u16,
    /// Предзаполненный адрес подключения (может быть пустым).
    pub dial: String,
    /// Состояние галки «запоминать настройки».
    pub remember: bool,
}

/// Запустить TUI. Блокирует до выхода пользователя или закрытия канала.
pub async fn run(
    to_engine: mpsc::UnboundedSender<UiCommand>,
    mut from_engine: mpsc::UnboundedReceiver<AppEvent>,
    me_fingerprint: String,
    startup: StartupConfig,
) -> Result<()> {
    let mut terminal = ratatui::init();
    let mut app = App::new(me_fingerprint, startup);
    let mut input_events = EventStream::new();

    let result = loop {
        if let Err(e) = terminal.draw(|f| view::draw(f, &app)) {
            break Err(e.into());
        }

        tokio::select! {
            maybe = input_events.next() => match maybe {
                Some(Ok(Event::Key(key))) if key.kind == KeyEventKind::Press => {
                    if let Some(cmd) = events::handle_key(&mut app, key) {
                        let _ = to_engine.send(cmd);
                    }
                }
                Some(Ok(_)) => {}
                Some(Err(e)) => break Err(e.into()),
                None => break Ok(()),
            },
            ev = from_engine.recv() => match ev {
                Some(ev) => app.apply(ev),
                None => break Ok(()),
            },
        }

        if app.should_quit {
            break Ok(());
        }
    };

    ratatui::restore();
    result
}
