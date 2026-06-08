//! Аргументы командной строки.

use clap::Parser;
use std::path::PathBuf;

/// Локальный P2P мессенджер со сквозным шифрованием.
#[derive(Debug, Clone, Parser)]
#[command(name = "p2p-chat", version, about)]
pub struct Config {
    /// UDP/TCP порт для входящих соединений. Если не задан в интерактивном
    /// режиме — спрашивается на стартовом экране TUI; иначе 0 (случайный).
    #[arg(short, long)]
    pub port: Option<u16>,

    /// Отображаемый никнейм (по умолчанию — имя пользователя ОС).
    #[arg(short, long)]
    pub nick: Option<String>,

    /// Адрес(а) пиров для ручного подключения. Принимает либо полный multiaddr
    /// (`/ip4/127.0.0.1/udp/4001/quic-v1`), либо короткий `ip:port`
    /// (трактуется как QUIC).
    #[arg(short, long = "dial", value_name = "ADDR")]
    pub dial: Vec<String>,

    /// Безголовый режим: вместо TUI читать команды из stdin и печатать события
    /// в stdout (для скриптов и тестов).
    #[arg(long)]
    pub headless: bool,

    /// Каталог данных (идентичность, логи, загрузки). Задайте разные каталоги,
    /// чтобы запускать несколько узлов на одной машине. По умолчанию —
    /// `$XDG_DATA_HOME/p2p-chat` (или переменная `P2P_CHAT_DATA_DIR`).
    #[arg(long, value_name = "DIR")]
    pub data_dir: Option<PathBuf>,
}

impl Config {
    /// Ник с подстановкой имени пользователя ОС, если не задан.
    pub fn nickname(&self) -> String {
        self.nick.clone().unwrap_or_else(default_nick)
    }

    /// Нужен ли интерактивный стартовый экран TUI. Показываем его только когда
    /// запуск «голый»: не headless и без флагов конфигурации.
    pub fn is_interactive(&self) -> bool {
        !self.headless && self.port.is_none() && self.nick.is_none() && self.dial.is_empty()
    }
}

/// Имя пользователя ОС как запасной ник.
pub fn default_nick() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "anon".to_string())
}
