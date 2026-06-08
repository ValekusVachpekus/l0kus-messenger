//! Wire-типы протокола `request-response` (`/p2p-chat/1`).
//!
//! Сериализуются CBOR-кодеком libp2p. Содержательная нагрузка передаётся уже
//! зашифрованной в варианте [`WireMsg::Encrypted`]; ключевой бандл — открытый
//! (он защищён транспортным шифрованием libp2p и сверяется по fingerprint).

use serde::{Deserialize, Serialize};

/// Имя протокола request-response.
pub const PROTOCOL: &str = "/p2p-chat/1";

/// Запрос (отправляемое сообщение).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WireMsg {
    /// Бандл ключей для установления Olm-сессии.
    KeyBundle {
        /// Долговременный Ed25519-ключ (base64) — основа fingerprint.
        ed25519: String,
        /// Curve25519 identity-ключ (base64).
        curve25519: String,
        /// Одноразовый Curve25519-ключ (base64) для X3DH.
        one_time_key: String,
        /// Отображаемый ник.
        nick: String,
    },
    /// Зашифрованная Olm-нагрузка: части OlmMessage `(type, body)`.
    Encrypted { ty: u8, body: Vec<u8> },
}

/// Ответ-подтверждение.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Ack {
    Ok,
    Err(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_roundtrip() {
        let msg = WireMsg::Encrypted {
            ty: 1,
            body: vec![1, 2, 3, 4],
        };
        let bytes = rmp_serde::to_vec(&msg).unwrap();
        let back: WireMsg = rmp_serde::from_slice(&bytes).unwrap();
        assert!(matches!(back, WireMsg::Encrypted { ty: 1, body } if body == vec![1,2,3,4]));

        let bundle = WireMsg::KeyBundle {
            ed25519: "a".into(),
            curve25519: "b".into(),
            one_time_key: "c".into(),
            nick: "alice".into(),
        };
        let bytes = rmp_serde::to_vec(&bundle).unwrap();
        let back: WireMsg = rmp_serde::from_slice(&bytes).unwrap();
        assert!(matches!(back, WireMsg::KeyBundle { nick, .. } if nick == "alice"));
    }
}
