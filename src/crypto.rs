//! Полезная нагрузка под Olm-шифрованием и помощники encrypt/decrypt.
//!
//! Всё, что отправляется между пирами по содержательным каналам, сначала
//! сериализуется в [`Plain`], затем шифруется Olm-сессией. Это держит wire-слой
//! (`net::protocol`) свободным от прикладной семантики.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use vodozemac::olm::{OlmMessage, Session};

/// Расшифрованная прикладная нагрузка.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Plain {
    /// Первый контрольный пакет при установлении сессии (не показывается).
    Hello { nick: String },
    /// Текстовое сообщение.
    Text(String),
    /// Предложение передать файл.
    FileOffer { id: u64, name: String, size: u64 },
    /// Очередной зашифрованный чанк файла. Несёт `total`, чтобы приём не зависел
    /// от порядка доставки и от прихода `FileOffer` (request-response не
    /// гарантирует порядок между отдельными запросами).
    FileChunk {
        id: u64,
        /// Смещение этого чанка в байтах от начала файла.
        offset: u64,
        /// Полный размер файла в байтах.
        total: u64,
        data: Vec<u8>,
    },
}

/// Зашифровать нагрузку в части OlmMessage `(type, body)` для wire-слоя.
pub fn seal(session: &mut Session, plain: &Plain) -> Result<(u8, Vec<u8>)> {
    let bytes = rmp_serde::to_vec(plain).context("сериализация Plain")?;
    let msg = session.encrypt(&bytes).context("шифрование Olm")?;
    let (ty, body) = msg.to_parts();
    Ok((ty as u8, body))
}

/// Расшифровать части `(type, body)` обратно в нагрузку.
pub fn open(session: &mut Session, ty: u8, body: &[u8]) -> Result<Plain> {
    let msg = OlmMessage::from_parts(ty as usize, body).context("разбор OlmMessage")?;
    let bytes = session.decrypt(&msg).context("расшифровка Olm")?;
    let plain = rmp_serde::from_slice(&bytes).context("разбор Plain")?;
    Ok(plain)
}

/// Восстановить [`OlmMessage`] из wire-частей (для создания inbound-сессии).
pub fn parse_message(ty: u8, body: &[u8]) -> Result<OlmMessage> {
    OlmMessage::from_parts(ty as usize, body).context("разбор OlmMessage")
}

#[cfg(test)]
mod tests {
    use super::*;
    use vodozemac::olm::{Account, SessionConfig};

    #[test]
    fn olm_roundtrip_via_plain() {
        // Боб создаёт аккаунт и публикует OTK.
        let mut bob = Account::new();
        bob.generate_one_time_keys(1);
        let bob_otk = *bob.one_time_keys().values().next().unwrap();
        let bob_identity = bob.curve25519_key();

        // Алиса создаёт outbound-сессию к Бобу и шлёт первый (PreKey) пакет.
        let alice = Account::new();
        let mut alice_session = alice
            .create_outbound_session(SessionConfig::default(), bob_identity, bob_otk)
            .unwrap();

        let (ty, body) = seal(&mut alice_session, &Plain::Text("привет".into())).unwrap();

        // Боб создаёт inbound-сессию из PreKey-сообщения.
        let msg = parse_message(ty, &body).unwrap();
        let OlmMessage::PreKey(prekey) = msg else {
            panic!("ожидался PreKey");
        };
        let result = bob
            .create_inbound_session(SessionConfig::default(), alice.curve25519_key(), &prekey)
            .unwrap();
        let mut bob_session = result.session;
        let first: Plain = rmp_serde::from_slice(&result.plaintext).unwrap();
        assert!(matches!(first, Plain::Text(t) if t == "привет"));

        // Обратное направление — обычное (Normal) сообщение.
        let (ty, body) = seal(&mut bob_session, &Plain::Text("здравствуй".into())).unwrap();
        let back = open(&mut alice_session, ty, &body).unwrap();
        assert!(matches!(back, Plain::Text(t) if t == "здравствуй"));
    }
}
