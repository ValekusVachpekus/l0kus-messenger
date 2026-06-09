//! Преобразование клавиш в команды engine и правки состояния ввода.

use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::app::{App, Screen, Who};
use crate::message::UiCommand;

/// Обработать нажатие клавиши. Возвращает команду для engine, если она нужна.
pub fn handle_key(app: &mut App, key: KeyEvent) -> Option<UiCommand> {
    match app.screen {
        Screen::Setup => handle_setup_key(app, key),
        Screen::Chat => handle_chat_key(app, key),
    }
}

// --- стартовый экран ---------------------------------------------------------

fn handle_setup_key(app: &mut App, key: KeyEvent) -> Option<UiCommand> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Esc => {
            app.should_quit = true;
            Some(UiCommand::Quit)
        }
        KeyCode::Char('c') if ctrl => {
            app.should_quit = true;
            Some(UiCommand::Quit)
        }
        KeyCode::Tab | KeyCode::Down => {
            app.setup.next_field();
            None
        }
        KeyCode::BackTab | KeyCode::Up => {
            app.setup.prev_field();
            None
        }
        KeyCode::Char(' ') => {
            app.setup.toggle();
            None
        }
        KeyCode::Enter => app.submit_setup(),
        KeyCode::Backspace => {
            app.setup.backspace();
            None
        }
        KeyCode::Char(c) => {
            app.setup.input_char(c);
            None
        }
        _ => None,
    }
}

// --- экран чата --------------------------------------------------------------

fn handle_chat_key(app: &mut App, key: KeyEvent) -> Option<UiCommand> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

    // Открытый файловый браузер перехватывает весь ввод.
    if app.file_browser.is_some() {
        return handle_browser_key(app, key);
    }

    // Оверлей помощи перехватывает ввод: любая клавиша его закрывает.
    if app.show_help {
        if matches!(key.code, KeyCode::Char('c')) && ctrl {
            app.should_quit = true;
            return Some(UiCommand::Quit);
        }
        app.show_help = false;
        return None;
    }

    match key.code {
        KeyCode::Esc => {
            app.should_quit = true;
            Some(UiCommand::Quit)
        }
        KeyCode::Char('c') if ctrl => {
            app.should_quit = true;
            Some(UiCommand::Quit)
        }
        KeyCode::Char('v') if ctrl => verify_selected(app),
        KeyCode::Char('f') if ctrl => {
            app.open_file_browser();
            None
        }
        KeyCode::Char('r') if ctrl => {
            app.status = "пересканирую LAN…".to_string();
            Some(UiCommand::RefreshDiscovery)
        }
        KeyCode::F(1) => {
            app.show_help = true;
            None
        }
        KeyCode::Char('?') if app.input.is_empty() => {
            app.show_help = true;
            None
        }
        KeyCode::PageUp => {
            app.scroll_up();
            None
        }
        KeyCode::PageDown => {
            app.scroll_down();
            None
        }
        KeyCode::Tab | KeyCode::Down => {
            app.select_next();
            None
        }
        KeyCode::Up => {
            app.select_prev();
            None
        }
        KeyCode::Backspace => {
            app.input.pop();
            None
        }
        KeyCode::Enter => submit(app),
        KeyCode::Char(c) => {
            app.input.push(c);
            None
        }
        _ => None,
    }
}

// --- файловый браузер --------------------------------------------------------

fn handle_browser_key(app: &mut App, key: KeyEvent) -> Option<UiCommand> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    if matches!(key.code, KeyCode::Char('c')) && ctrl {
        app.should_quit = true;
        return Some(UiCommand::Quit);
    }
    let browser = app.file_browser.as_mut()?;
    match key.code {
        KeyCode::Esc => {
            app.file_browser = None;
            None
        }
        KeyCode::Up => {
            browser.up();
            None
        }
        KeyCode::Down => {
            browser.down();
            None
        }
        // На уровень выше.
        KeyCode::Backspace | KeyCode::Left => {
            browser.parent();
            None
        }
        // Войти в каталог или выбрать файл.
        KeyCode::Enter | KeyCode::Right => {
            let peer = browser.peer;
            match browser.activate() {
                Some(path) => {
                    app.file_browser = None;
                    app.push_to_selected(
                        Who::System,
                        format!("отправка файла {}…", path.display()),
                    );
                    Some(UiCommand::SendFile { peer, path })
                }
                None => None,
            }
        }
        _ => None,
    }
}

fn verify_selected(app: &mut App) -> Option<UiCommand> {
    let peer = app.selected_peer_id()?;
    // Обновляем состояние TUI сразу — engine хранит свой флаг отдельно.
    if let Some(p) = app.peers.get_mut(app.selected) {
        p.verified = true;
    }
    Some(UiCommand::VerifyPeer { peer })
}

fn submit(app: &mut App) -> Option<UiCommand> {
    let line = std::mem::take(&mut app.input);
    let line = line.trim();
    if line.is_empty() {
        return None;
    }

    if let Some(addr) = line.strip_prefix("/dial ") {
        return Some(UiCommand::Dial {
            addr: addr.trim().to_string(),
        });
    }
    if line == "/verify" {
        return verify_selected(app);
    }
    if line == "/refresh" {
        app.status = "пересканирую LAN…".to_string();
        return Some(UiCommand::RefreshDiscovery);
    }
    if let Some(path) = line.strip_prefix("/file ") {
        let peer = match app.selected_peer_id() {
            Some(p) => p,
            None => {
                app.status = "нет выбранного пира для файла".to_string();
                return None;
            }
        };
        let path = PathBuf::from(shellexpand_tilde(path.trim()));
        app.push_to_selected(Who::System, format!("отправка файла {}…", path.display()));
        return Some(UiCommand::SendFile { peer, path });
    }

    // Обычный текст выбранному пиру.
    match app.selected_peer_id() {
        Some(peer) => {
            app.push_to_selected(Who::Me, line.to_string());
            Some(UiCommand::SendText {
                peer,
                text: line.to_string(),
            })
        }
        None => {
            app.status = "нет выбранного пира — некому отправить".to_string();
            None
        }
    }
}

/// Простая подстановка `~` в начале пути.
fn shellexpand_tilde(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/")
        && let Some(home) = std::env::var_os("HOME")
    {
        return format!("{}/{}", home.to_string_lossy(), rest);
    }
    path.to_string()
}
