//! Композиция сетевого поведения libp2p.

use libp2p::swarm::NetworkBehaviour;
use libp2p::{StreamProtocol, identify, mdns, ping, request_response};

use super::protocol::{Ack, PROTOCOL, WireMsg};

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
        let mdns = mdns::tokio::Behaviour::new(mdns::Config::default(), peer_id)?;
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
