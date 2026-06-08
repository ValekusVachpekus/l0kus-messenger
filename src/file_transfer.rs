//! Чанкинг исходящих файлов и сборка входящих.
//!
//! Сами чанки шифруются Olm-сессией (см. [`crate::crypto::Plain::FileChunk`]);
//! здесь только разбиение/сборка и запись на диск.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// Размер чанка файла (до шифрования).
const CHUNK_SIZE: usize = 16 * 1024;

/// Готовый к отправке файл, разбитый на чанки `(offset, bytes)`.
pub struct FileSend {
    pub name: String,
    pub size: u64,
    pub chunks: Vec<(u64, Vec<u8>)>,
}

/// Прочитать файл и разбить на чанки.
pub fn prepare(path: &Path) -> Result<FileSend> {
    let data = std::fs::read(path).with_context(|| format!("чтение {}", path.display()))?;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".to_string());
    let size = data.len() as u64;
    let mut chunks = Vec::new();
    let mut offset = 0u64;
    for chunk in data.chunks(CHUNK_SIZE) {
        chunks.push((offset, chunk.to_vec()));
        offset += chunk.len() as u64;
    }
    // Пустой файл — один пустой чанк, чтобы передача завершилась корректно.
    if chunks.is_empty() {
        chunks.push((0, Vec::new()));
    }
    Ok(FileSend { name, size, chunks })
}

/// Накопитель входящего файла. Устойчив к доставке чанков не по порядку и к
/// тому, что `FileOffer` может прийти после первых чанков.
#[derive(Default)]
pub struct IncomingFile {
    name: Option<String>,
    total: Option<u64>,
    buf: Vec<u8>,
    /// Сколько байт реально записано (сумма длин полученных чанков).
    filled: u64,
}

impl IncomingFile {
    pub fn name(&self) -> String {
        self.name.clone().unwrap_or_else(|| "file".to_string())
    }

    pub fn total(&self) -> u64 {
        self.total.unwrap_or(self.filled)
    }

    pub fn received(&self) -> u64 {
        self.filled
    }

    /// Задать имя (из `FileOffer`).
    pub fn set_name(&mut self, name: String) {
        self.name = Some(name);
    }

    /// Задать полный размер (из `FileOffer` или из чанка).
    pub fn set_total(&mut self, total: u64) {
        self.total = Some(total);
    }

    /// Передача завершена, когда известен размер и все байты получены.
    pub fn is_complete(&self) -> bool {
        matches!(self.total, Some(t) if self.filled >= t)
    }

    /// Дописать чанк по смещению (любой порядок).
    pub fn push(&mut self, offset: u64, data: &[u8]) {
        let offset = offset as usize;
        let end = offset + data.len();
        if end > self.buf.len() {
            self.buf.resize(end, 0);
        }
        self.buf[offset..end].copy_from_slice(data);
        self.filled += data.len() as u64;
    }

    /// Сохранить собранный файл в каталог загрузок, вернуть путь.
    pub fn finish(&self) -> Result<PathBuf> {
        let dir = downloads_dir()?;
        std::fs::create_dir_all(&dir).ok();
        let path = unique_path(&dir, &self.name());
        std::fs::write(&path, &self.buf)
            .with_context(|| format!("запись {}", path.display()))?;
        Ok(path)
    }
}

/// Каталог загрузок: `$XDG_DATA_HOME/p2p-chat/downloads`.
pub fn downloads_dir() -> Result<PathBuf> {
    Ok(crate::identity::data_dir()?.join("downloads"))
}

/// Подобрать несуществующее имя файла (добавляя суффиксы при коллизии).
fn unique_path(dir: &Path, name: &str) -> PathBuf {
    let candidate = dir.join(name);
    if !candidate.exists() {
        return candidate;
    }
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) => (s.to_string(), format!(".{e}")),
        None => (name.to_string(), String::new()),
    };
    for i in 1.. {
        let candidate = dir.join(format!("{stem} ({i}){ext}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!()
}
