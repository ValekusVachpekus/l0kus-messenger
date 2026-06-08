//! Доменные типы и сообщения каналов между TUI и engine.

use libp2p::PeerId;
use std::path::PathBuf;

/// Команды от TUI к engine.
#[derive(Debug)]
pub enum UiCommand {
    /// Запустить узел: начать слушать на порту, задать ник и набрать адреса.
    /// Шлётся один раз — со стартового экрана TUI или сразу из main (флаги).
    Start {
        nick: String,
        port: u16,
        dials: Vec<String>,
    },
    /// Отправить текстовое сообщение пиру.
    SendText { peer: PeerId, text: String },
    /// Отправить файл пиру.
    SendFile { peer: PeerId, path: PathBuf },
    /// Пометить пира доверенным (пользователь сверил fingerprint).
    VerifyPeer { peer: PeerId },
    /// Подключиться к адресу вручную.
    Dial { addr: String },
    /// Завершить работу.
    Quit,
}

/// Состояние присутствия пира.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presence {
    Online,
    Offline,
}

/// События от engine к TUI.
#[derive(Debug, Clone)]
pub enum AppEvent {
    /// Наш собственный адрес для прослушивания (для подсказки пользователю).
    Listening { addr: String },
    /// Обнаружен/представился новый пир.
    PeerDiscovered {
        peer: PeerId,
        nick: String,
        /// Fingerprint долговременного ключа для ручной сверки.
        fingerprint: String,
    },
    /// Изменилось присутствие пира.
    PeerPresence { peer: PeerId, presence: Presence },
    /// Принято расшифрованное текстовое сообщение.
    MessageReceived {
        peer: PeerId,
        text: String,
    },
    /// Прогресс/событие передачи файла.
    FileProgress {
        peer: PeerId,
        name: String,
        received: u64,
        total: u64,
        /// Путь сохранённого файла, когда передача завершена.
        done: Option<PathBuf>,
        outgoing: bool,
    },
    /// Информационное/системное сообщение для статуса.
    Status(String),
}
