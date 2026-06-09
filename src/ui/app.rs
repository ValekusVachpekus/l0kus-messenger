//! Состояние TUI (эфемерное — живёт только в памяти сессии).

use libp2p::PeerId;

use crate::message::{AppEvent, Presence, UiCommand};
use crate::settings::Settings;

use super::StartupConfig;
use super::files::FileBrowser;

/// Кто автор строки чата.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Who {
    Me,
    Them,
    System,
}

#[derive(Debug, Clone)]
pub struct ChatLine {
    pub who: Who,
    pub text: String,
    /// Локальное время `HH:MM` создания строки.
    pub time: String,
}

impl ChatLine {
    pub fn new(who: Who, text: String) -> Self {
        ChatLine {
            who,
            text,
            time: now_hhmm(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PeerEntry {
    pub id: PeerId,
    pub nick: String,
    pub fingerprint: String,
    pub online: bool,
    pub verified: bool,
    pub messages: Vec<ChatLine>,
}

/// Какой экран показываем.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Setup,
    Chat,
}

/// Поля стартового экрана настройки.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Nick,
    Port,
    Dial,
    Remember,
}

const FIELDS: [Field; 4] = [Field::Nick, Field::Port, Field::Dial, Field::Remember];

/// Состояние стартовой формы.
#[derive(Debug, Clone)]
pub struct SetupForm {
    pub nick: String,
    pub port: String,
    pub dial: String,
    pub remember: bool,
    pub field: usize,
    pub error: Option<String>,
}

impl SetupForm {
    pub fn current(&self) -> Field {
        FIELDS[self.field]
    }

    pub fn next_field(&mut self) {
        self.field = (self.field + 1) % FIELDS.len();
    }

    pub fn prev_field(&mut self) {
        self.field = (self.field + FIELDS.len() - 1) % FIELDS.len();
    }

    /// Текущее редактируемое текстовое поле (None для чекбокса).
    fn text_field_mut(&mut self) -> Option<&mut String> {
        match self.current() {
            Field::Nick => Some(&mut self.nick),
            Field::Port => Some(&mut self.port),
            Field::Dial => Some(&mut self.dial),
            Field::Remember => None,
        }
    }

    pub fn input_char(&mut self, c: char) {
        let is_port = self.current() == Field::Port;
        if let Some(f) = self.text_field_mut() {
            // В поле порта принимаем только цифры.
            if !is_port || c.is_ascii_digit() {
                f.push(c);
            }
        }
    }

    pub fn backspace(&mut self) {
        if let Some(f) = self.text_field_mut() {
            f.pop();
        }
    }

    pub fn toggle(&mut self) {
        if self.current() == Field::Remember {
            self.remember = !self.remember;
        }
    }
}

pub struct App {
    pub me_nick: String,
    pub me_fingerprint: String,
    pub listen_addrs: Vec<String>,
    pub peers: Vec<PeerEntry>,
    pub selected: usize,
    pub input: String,
    pub status: String,
    pub should_quit: bool,
    pub screen: Screen,
    pub setup: SetupForm,
    pub show_help: bool,
    /// Прокрутка чата вверх в строках от низа (0 — прилипли к низу).
    pub scroll: usize,
    /// Открытый файловый браузер (выбор файла на отправку), если активен.
    pub file_browser: Option<FileBrowser>,
}

impl App {
    pub fn new(me_fingerprint: String, startup: StartupConfig) -> Self {
        let screen = if startup.interactive {
            Screen::Setup
        } else {
            Screen::Chat
        };
        let setup = SetupForm {
            nick: startup.nick.clone(),
            port: if startup.port == 0 {
                String::new()
            } else {
                startup.port.to_string()
            },
            dial: startup.dial,
            remember: startup.remember,
            field: 0,
            error: None,
        };
        App {
            me_nick: startup.nick,
            me_fingerprint,
            listen_addrs: Vec::new(),
            peers: Vec::new(),
            selected: 0,
            input: String::new(),
            status: if startup.interactive {
                "настройте узел и нажмите Enter".to_string()
            } else {
                "ожидание пиров…".to_string()
            },
            should_quit: false,
            screen,
            setup,
            show_help: false,
            scroll: 0,
            file_browser: None,
        }
    }

    /// Открыть файловый браузер для отправки файла выбранному пиру.
    pub fn open_file_browser(&mut self) {
        if let Some(peer) = self.selected_peer_id() {
            self.file_browser = Some(FileBrowser::open(peer));
        } else {
            self.status = "нет выбранного пира для файла".to_string();
        }
    }

    /// Подтвердить стартовую форму: проверить ввод, при необходимости сохранить
    /// настройки, перейти в чат и вернуть команду `Start` для engine.
    pub fn submit_setup(&mut self) -> Option<UiCommand> {
        let nick = self.setup.nick.trim().to_string();
        let nick = if nick.is_empty() {
            crate::config::default_nick()
        } else {
            nick
        };
        let port_str = self.setup.port.trim();
        let port: u16 = if port_str.is_empty() {
            0
        } else {
            match port_str.parse() {
                Ok(p) => p,
                Err(_) => {
                    self.setup.error = Some("порт должен быть числом 0–65535".to_string());
                    return None;
                }
            }
        };
        let dial = self.setup.dial.trim().to_string();
        let dials: Vec<String> = if dial.is_empty() { Vec::new() } else { vec![dial] };

        if self.setup.remember {
            Settings {
                nick: Some(nick.clone()),
                port,
                dial: dials.clone(),
                remember: true,
            }
            .save();
        }

        self.me_nick = nick.clone();
        self.screen = Screen::Chat;
        self.setup.error = None;
        Some(UiCommand::Start { nick, port, dials })
    }

    pub fn selected_peer(&self) -> Option<&PeerEntry> {
        self.peers.get(self.selected)
    }

    pub fn selected_peer_id(&self) -> Option<PeerId> {
        self.peers.get(self.selected).map(|p| p.id)
    }

    pub fn select_next(&mut self) {
        if !self.peers.is_empty() {
            self.selected = (self.selected + 1) % self.peers.len();
            self.scroll = 0;
        }
    }

    pub fn select_prev(&mut self) {
        if !self.peers.is_empty() {
            self.selected = (self.selected + self.peers.len() - 1) % self.peers.len();
            self.scroll = 0;
        }
    }

    pub fn scroll_up(&mut self) {
        self.scroll = self.scroll.saturating_add(3);
    }

    pub fn scroll_down(&mut self) {
        self.scroll = self.scroll.saturating_sub(3);
    }

    fn peer_mut(&mut self, id: PeerId) -> &mut PeerEntry {
        if let Some(idx) = self.peers.iter().position(|p| p.id == id) {
            return &mut self.peers[idx];
        }
        self.peers.push(PeerEntry {
            id,
            nick: short_id(&id),
            fingerprint: String::new(),
            online: false,
            verified: false,
            messages: Vec::new(),
        });
        self.peers.last_mut().unwrap()
    }

    /// Записать строку в чат выбранного пира (например, наше отправленное).
    pub fn push_to_selected(&mut self, who: Who, text: String) {
        if let Some(p) = self.peers.get_mut(self.selected) {
            p.messages.push(ChatLine::new(who, text));
        }
        self.scroll = 0;
    }

    /// Применить событие от engine.
    pub fn apply(&mut self, ev: AppEvent) {
        match ev {
            AppEvent::Listening { addr } => {
                if !self.listen_addrs.contains(&addr) {
                    self.listen_addrs.push(addr);
                }
            }
            AppEvent::PeerDiscovered {
                peer,
                nick,
                fingerprint,
            } => {
                let p = self.peer_mut(peer);
                p.nick = nick;
                p.fingerprint = fingerprint;
            }
            AppEvent::PeerPresence { peer, presence } => {
                let p = self.peer_mut(peer);
                p.online = presence == Presence::Online;
            }
            AppEvent::MessageReceived { peer, text } => {
                let p = self.peer_mut(peer);
                p.messages.push(ChatLine::new(Who::Them, text));
                self.scroll = 0;
            }
            AppEvent::FileProgress {
                peer,
                name,
                received,
                total,
                done,
                outgoing,
            } => {
                let f = super::theme::ICON_FILE;
                let line = match (&done, outgoing) {
                    (Some(path), _) if !outgoing => {
                        format!("{f} получен файл «{name}» → {}", path.display())
                    }
                    (_, true) => format!("{f} отправлен файл «{name}» ({total} Б)"),
                    _ => format!("{f} приём «{name}»: {received}/{total} Б"),
                };
                let p = self.peer_mut(peer);
                p.messages.push(ChatLine::new(Who::System, line));
            }
            AppEvent::Status(msg) => {
                self.status = msg;
            }
        }
    }
}

/// Текущее локальное время в формате `HH:MM`.
fn now_hhmm() -> String {
    chrono::Local::now().format("%H:%M").to_string()
}

/// Короткое представление PeerId (последние 8 символов).
pub fn short_id(id: &PeerId) -> String {
    let s = id.to_string();
    let tail = &s[s.len().saturating_sub(8)..];
    format!("peer-{tail}")
}
