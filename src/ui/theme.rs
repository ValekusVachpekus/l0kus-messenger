//! Цветовая тема TUI — «неоновый циан».
//!
//! Палитра задаётся через `Color::Rgb`, чтобы вид не зависел от 16-цветной
//! схемы терминала. Здесь же — мелкие хелперы стилей.

use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, BorderType, Borders};

/// Яркий бирюзовый акцент (рамки, заголовки, наш ввод).
pub const ACCENT: Color = Color::Rgb(0x22, 0xD3, 0xEE);
/// Приглушённый акцент (неактивные рамки, подсказки).
pub const ACCENT_DIM: Color = Color::Rgb(0x0E, 0x74, 0x90);
/// Мадженту для второстепенных деталей (входящие, fingerprint).
pub const MAGENTA: Color = Color::Rgb(0xC0, 0x84, 0xFC);
/// Онлайн-индикатор.
pub const ONLINE: Color = Color::Rgb(0x4A, 0xDE, 0x80);
/// Оффлайн-индикатор / выключенное.
pub const OFFLINE: Color = Color::Rgb(0x52, 0x5B, 0x6B);
/// Основной текст.
pub const TEXT: Color = Color::Rgb(0xE6, 0xED, 0xF3);
/// Приглушённый текст (статус, системные строки).
pub const MUTED: Color = Color::Rgb(0x7D, 0x8A, 0x99);
/// Фон выделенной строки в списке пиров.
pub const SEL_BG: Color = Color::Rgb(0x0B, 0x39, 0x49);
/// Предупреждение / «не сверено».
pub const WARN: Color = Color::Rgb(0xF5, 0xC2, 0x44);

/// Скруглённый блок с акцентным заголовком (заголовок копируется — `'static`).
pub fn panel(title: &str) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT_DIM))
        .title_style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD))
        .title(format!(" ◈ {title} "))
}

/// Тот же блок, но с ярко-акцентной рамкой (для активного элемента).
pub fn panel_active(title: &str) -> Block<'static> {
    panel(title).border_style(Style::default().fg(ACCENT))
}
