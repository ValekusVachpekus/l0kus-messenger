//! Долговременная криптографическая идентичность узла на базе vodozemac.
//!
//! Ключевая пара персистится на диск, поэтому fingerprint стабилен между
//! запусками. Сообщения при этом остаются эфемерными.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use vodozemac::Curve25519PublicKey;
use vodozemac::olm::{
    Account, AccountPickle, InboundCreationResult, PreKeyMessage, Session, SessionConfig,
};

pub struct Identity {
    account: Account,
    nick: String,
    path: PathBuf,
}

impl Identity {
    /// Загрузить идентичность с диска либо создать новую и сохранить.
    pub fn load_or_create(nick: String) -> Result<Self> {
        let path = identity_path()?;
        let account = if path.exists() {
            let bytes = std::fs::read(&path)
                .with_context(|| format!("чтение идентичности {}", path.display()))?;
            let pickle: AccountPickle =
                rmp_serde::from_slice(&bytes).context("разбор pickle идентичности")?;
            Account::from_pickle(pickle)
        } else {
            Account::new()
        };

        let id = Identity {
            account,
            nick,
            path,
        };
        id.persist()?;
        Ok(id)
    }

    pub fn nick(&self) -> &str {
        &self.nick
    }

    /// Сменить отображаемый ник (в `identity.bin` не хранится — безопасно).
    pub fn set_nick(&mut self, nick: String) {
        if !nick.is_empty() {
            self.nick = nick;
        }
    }

    pub fn identity_curve(&self) -> Curve25519PublicKey {
        self.account.curve25519_key()
    }

    /// Base64 долговременного Ed25519-ключа.
    pub fn identity_ed_base64(&self) -> String {
        self.account.ed25519_key().to_base64()
    }

    /// Человекочитаемый fingerprint для ручной сверки (группы по 4 символа).
    pub fn fingerprint(&self) -> String {
        fingerprint_of(&self.identity_ed_base64())
    }

    /// Сгенерировать и выдать свежий публичный one-time ключ для нового пира.
    ///
    /// Каждому пиру — отдельный OTK (гарантия уникальности без учёта пула).
    /// Публикацию не делаем: приватные части остаются в аккаунте, чтобы
    /// [`Self::create_inbound`] могла найти соответствие. Приватный ключ
    /// персистится, поэтому переживает перезапуск (для отложенных inbound).
    pub fn take_one_time_key(&mut self) -> Curve25519PublicKey {
        let result = self.account.generate_one_time_keys(1);
        let key = result
            .created
            .into_iter()
            .next()
            .expect("generate_one_time_keys(1) создаёт ровно один ключ");
        let _ = self.persist();
        key
    }

    /// Создать исходящую (outbound) Olm-сессию к пиру.
    pub fn create_outbound(
        &self,
        peer_identity: Curve25519PublicKey,
        peer_otk: Curve25519PublicKey,
    ) -> Result<Session> {
        self.account
            .create_outbound_session(SessionConfig::default(), peer_identity, peer_otk)
            .context("создание outbound Olm-сессии")
    }

    /// Создать входящую (inbound) сессию из PreKey-сообщения.
    pub fn create_inbound(
        &mut self,
        their_identity: Curve25519PublicKey,
        prekey: &PreKeyMessage,
    ) -> Result<(Session, Vec<u8>)> {
        let InboundCreationResult { session, plaintext } = self
            .account
            .create_inbound_session(SessionConfig::default(), their_identity, prekey)
            .context("создание inbound Olm-сессии")?;
        // OTK израсходован — сохраняем обновлённый аккаунт.
        self.persist()?;
        Ok((session, plaintext))
    }

    fn persist(&self) -> Result<()> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).ok();
        }
        let bytes = rmp_serde::to_vec(&self.account.pickle()).context("сериализация идентичности")?;
        std::fs::write(&self.path, bytes)
            .with_context(|| format!("запись идентичности {}", self.path.display()))?;
        set_private_perms(&self.path);
        Ok(())
    }
}

/// Каталог данных. По умолчанию `$XDG_DATA_HOME/p2p-chat`; можно переопределить
/// переменной `P2P_CHAT_DATA_DIR` (удобно для запуска нескольких узлов на одной
/// машине и для тестов).
pub fn data_dir() -> Result<PathBuf> {
    if let Some(dir) = std::env::var_os("P2P_CHAT_DATA_DIR") {
        return Ok(PathBuf::from(dir));
    }
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .context("не удалось определить домашнюю директорию")?;
    Ok(base.join("p2p-chat"))
}

fn identity_path() -> Result<PathBuf> {
    Ok(data_dir()?.join("identity.bin"))
}

/// Форматирует base64-ключ в fingerprint (группы по 4 символа) для сверки.
pub fn fingerprint_of(b64: &str) -> String {
    b64.as_bytes()
        .chunks(4)
        .map(|c| std::str::from_utf8(c).unwrap_or(""))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(unix)]
fn set_private_perms(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
}

#[cfg(not(unix))]
fn set_private_perms(_path: &Path) {}
