//! Сдержанная цветовая тема TUI.
//!
//! Намеренно используем стандартные 16 цветов терминала (а не RGB-неон), чтобы
//! интерфейс был строгим и совпадал с палитрой терминала пользователя. Те же
//! цвета применяются к подсветке файлов в браузере (как `ls`).

use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders};

/// Спокойный акцент (заголовки, активная рамка, наш ввод).
pub const ACCENT: Color = Color::Cyan;
/// Обычные рамки.
pub const BORDER: Color = Color::DarkGray;
/// Основной текст — цвет терминала по умолчанию.
pub const TEXT: Color = Color::Reset;
/// Приглушённый текст (время, подсказки, системные строки).
pub const MUTED: Color = Color::DarkGray;
/// Онлайн-индикатор.
pub const ONLINE: Color = Color::Green;
/// Оффлайн-индикатор.
pub const OFFLINE: Color = Color::DarkGray;
/// Предупреждение / «не сверено».
pub const WARN: Color = Color::Yellow;
/// Фон выделенной строки.
pub const SEL_BG: Color = Color::Indexed(238);

// ls-подобные цвета записей файлового браузера.
pub const DIR: Color = Color::Blue;
pub const EXEC: Color = Color::Green;
pub const LINK: Color = Color::Cyan;

// Иконки Nerd Font (нужен патченный шрифт в терминале — как и положено TUI).
pub const ICON_ONLINE: &str = "\u{f111}"; //
pub const ICON_OFFLINE: &str = "\u{f10c}"; //
pub const ICON_VERIFIED: &str = "\u{f00c}"; //
pub const ICON_FILE: &str = "\u{f0c6}"; //
pub const ICON_WARN: &str = "\u{f071}"; //
pub const ICON_DIR: &str = "\u{f07b}"; //
pub const ICON_FILE_PLAIN: &str = "\u{f15b}"; //
pub const ICON_EXEC: &str = "\u{f120}"; //
pub const ICON_LINK: &str = "\u{f0c1}"; //

/// Блок с серой рамкой и акцентным заголовком.
pub fn panel(title: &str) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(BORDER))
        .title_style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD))
        .title(format!(" {title} "))
}

/// Тот же блок, но с акцентной рамкой (для активного элемента).
pub fn panel_active(title: &str) -> Block<'static> {
    panel(title).border_style(Style::default().fg(ACCENT))
}
