//! Необязательное сохранение настроек запуска (`config.toml` в каталоге данных).
//!
//! Хранит выбор пользователя со стартового экрана TUI (ник/порт/адрес), чтобы
//! предзаполнять форму при следующем запуске. Идентичность и история тут ни при
//! чём — это только удобство ввода. Ошибки чтения/записи не фатальны.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::identity;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub nick: Option<String>,
    #[serde(default)]
    pub port: u16,
    #[serde(default)]
    pub dial: Vec<String>,
    /// Была ли включена галка «запоминать» (для предзаполнения чекбокса).
    #[serde(default)]
    pub remember: bool,
}

impl Settings {
    /// Прочитать из `config.toml`; при любой ошибке вернуть значения по умолчанию.
    pub fn load() -> Settings {
        let Ok(path) = config_path() else {
            return Settings::default();
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Settings::default();
        };
        toml::from_str(&text).unwrap_or_default()
    }

    /// Сохранить в `config.toml`. Ошибки логируются, но не прерывают работу.
    pub fn save(&self) {
        let Ok(path) = config_path() else { return };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).ok();
        }
        match toml::to_string_pretty(self) {
            Ok(text) => {
                if let Err(e) = std::fs::write(&path, text) {
                    tracing::warn!("не удалось сохранить настройки: {e}");
                }
            }
            Err(e) => tracing::warn!("сериализация настроек не удалась: {e}"),
        }
    }
}

fn config_path() -> anyhow::Result<PathBuf> {
    Ok(identity::data_dir()?.join("config.toml"))
}
