//! Простой файловый браузер для выбора файла на отправку.
//!
//! Навигация по каталогам с ls-подобной подсветкой: директории, исполняемые
//! файлы и символические ссылки выделяются цветом (см. `view::draw_file_browser`).

use std::path::{Path, PathBuf};

use libp2p::PeerId;

/// Тип записи каталога (для подсветки, как в терминале).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    Dir,
    Exec,
    Link,
    File,
}

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub name: String,
    pub path: PathBuf,
    pub kind: EntryKind,
    /// Можно ли «войти» (директория или ссылка на директорию).
    pub is_dir: bool,
}

/// Состояние браузера: целевой пир, текущий каталог и список записей.
pub struct FileBrowser {
    pub peer: PeerId,
    pub cwd: PathBuf,
    pub entries: Vec<FileEntry>,
    pub selected: usize,
    pub error: Option<String>,
}

impl FileBrowser {
    /// Открыть браузер для отправки файла `peer`. Старт — домашний каталог.
    pub fn open(peer: PeerId) -> Self {
        let cwd = std::env::var_os("HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("/"));
        let mut b = FileBrowser {
            peer,
            cwd,
            entries: Vec::new(),
            selected: 0,
            error: None,
        };
        b.refresh();
        b
    }

    fn refresh(&mut self) {
        self.selected = 0;
        match read_dir(&self.cwd) {
            Ok(list) => {
                self.entries = list;
                self.error = None;
            }
            Err(e) => {
                self.entries.clear();
                self.error = Some(e);
            }
        }
    }

    pub fn up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn down(&mut self) {
        if self.selected + 1 < self.entries.len() {
            self.selected += 1;
        }
    }

    /// Перейти в родительский каталог, выделив каталог, из которого вышли.
    pub fn parent(&mut self) {
        let Some(parent) = self.cwd.parent().map(Path::to_path_buf) else {
            return;
        };
        let from = self.cwd.file_name().map(|s| s.to_string_lossy().to_string());
        self.cwd = parent;
        self.refresh();
        if let Some(name) = from
            && let Some(i) = self.entries.iter().position(|e| e.name == name)
        {
            self.selected = i;
        }
    }

    /// Войти в выбранную директорию или вернуть путь файла для отправки.
    pub fn activate(&mut self) -> Option<PathBuf> {
        let entry = self.entries.get(self.selected)?;
        if entry.is_dir {
            self.cwd = entry.path.clone();
            self.refresh();
            None
        } else {
            Some(entry.path.clone())
        }
    }
}

/// Прочитать каталог: скрытые файлы пропускаем (как `ls` без `-a`), директории
/// идут первыми, далее — по алфавиту без учёта регистра.
fn read_dir(dir: &Path) -> Result<Vec<FileEntry>, String> {
    let rd = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut out: Vec<FileEntry> = Vec::new();
    for ent in rd.flatten() {
        let name = ent.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let path = ent.path();
        let is_link = ent.file_type().map(|t| t.is_symlink()).unwrap_or(false);
        // metadata() следует за ссылкой — определяем, директория ли это по факту.
        let meta = std::fs::metadata(&path).ok();
        let is_dir = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);
        let kind = if is_link {
            EntryKind::Link
        } else if is_dir {
            EntryKind::Dir
        } else if is_exec(meta.as_ref()) {
            EntryKind::Exec
        } else {
            EntryKind::File
        };
        out.push(FileEntry {
            name,
            path,
            kind,
            is_dir,
        });
    }
    out.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(out)
}

#[cfg(unix)]
fn is_exec(meta: Option<&std::fs::Metadata>) -> bool {
    use std::os::unix::fs::PermissionsExt;
    meta.map(|m| m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_exec(_meta: Option<&std::fs::Metadata>) -> bool {
    false
}
