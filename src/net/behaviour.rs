//! Композиция сетевого поведения libp2p.

use std::time::Duration;

use libp2p::swarm::NetworkBehaviour;
use libp2p::{PeerId, StreamProtocol, identify, mdns, ping, request_response};

use super::protocol::{Ack, PROTOCOL, WireMsg};

/// Как часто mDNS рассылает запрос обнаружения. Дефолт libp2p — 5 минут, из-за
/// чего поздно подключившийся узел мог несколько минут оставаться невидимым.
/// Короткий интервал делает повторное обнаружение быстрым.
const MDNS_QUERY_INTERVAL: Duration = Duration::from_secs(30);

/// Собрать mDNS-поведение с укороченным интервалом запроса. Вынесено отдельно,
/// чтобы движок мог пересоздать его (сброс кэша обнаруженных + новый опрос) при
/// запуске узла и по ручному обновлению.
pub fn make_mdns(peer_id: PeerId) -> std::io::Result<mdns::tokio::Behaviour> {
    let cfg = mdns::Config {
        query_interval: MDNS_QUERY_INTERVAL,
        ..mdns::Config::default()
    };
    mdns::tokio::Behaviour::new(cfg, peer_id)
}

/// Сводное поведение узла.
#[derive(NetworkBehaviour)]
pub struct Behaviour {
    /// Автообнаружение пиров в LAN.
    pub mdns: mdns::tokio::Behaviour,
    /// Обмен сведениями о пире (PeerId, адреса, протоколы).
    pub identify: identify::Behaviour,
    /// Keepalive и присутствие.
    pub ping: ping::Behaviour,
    /// Доставка прикладных сообщений.
    pub rr: request_response::cbor::Behaviour<WireMsg, Ack>,
}

impl Behaviour {
    pub fn new(key: &libp2p::identity::Keypair) -> anyhow::Result<Self> {
        let peer_id = key.public().to_peer_id();
        let mdns = make_mdns(peer_id)?;
        let identify = identify::Behaviour::new(identify::Config::new(
            PROTOCOL.to_string(),
            key.public(),
        ));
        let ping = ping::Behaviour::new(ping::Config::new());
        let rr = request_response::cbor::Behaviour::new(
            [(
                StreamProtocol::new(PROTOCOL),
                request_response::ProtocolSupport::Full,
            )],
            request_response::Config::default(),
        );
        Ok(Behaviour {
            mdns,
            identify,
            ping,
            rr,
        })
    }
}
