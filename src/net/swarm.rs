//! Сборка libp2p Swarm и разбор адресов.

use std::time::Duration;

use anyhow::{Context, Result};
use libp2p::{Multiaddr, Swarm, multiaddr::Protocol, noise, tcp, yamux};

use super::behaviour::Behaviour;

/// Построить Swarm с QUIC (основной) и TCP (запасной) транспортами.
pub fn build() -> Result<Swarm<Behaviour>> {
    let swarm = libp2p::SwarmBuilder::with_new_identity()
        .with_tokio()
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        )
        .context("настройка TCP-транспорта")?
        .with_quic()
        .with_behaviour(|key| Behaviour::new(key).expect("сборка поведения"))
        .map_err(|e| anyhow::anyhow!("сборка поведения: {e}"))?
        .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(120)))
        .build();
    Ok(swarm)
}

/// Начать слушать на указанном порту по QUIC и TCP (0 — случайный порт).
pub fn listen(swarm: &mut Swarm<Behaviour>, port: u16) -> Result<()> {
    let quic: Multiaddr = format!("/ip4/0.0.0.0/udp/{port}/quic-v1")
        .parse()
        .context("разбор QUIC-адреса")?;
    let tcp: Multiaddr = format!("/ip4/0.0.0.0/tcp/{port}")
        .parse()
        .context("разбор TCP-адреса")?;
    swarm.listen_on(quic)?;
    swarm.listen_on(tcp)?;
    Ok(())
}

/// Разобрать строку подключения: полный multiaddr или короткий `ip:port`
/// (трактуется как QUIC).
pub fn parse_dial(addr: &str) -> Result<Multiaddr> {
    if addr.starts_with('/') {
        return addr.parse().context("разбор multiaddr");
    }
    let (ip, port) = addr
        .rsplit_once(':')
        .context("ожидался формат ip:port или /ip4/.../quic-v1")?;
    let ip: std::net::Ipv4Addr = ip.parse().context("разбор IP")?;
    let port: u16 = port.parse().context("разбор порта")?;
    Ok(Multiaddr::empty()
        .with(Protocol::Ip4(ip))
        .with(Protocol::Udp(port))
        .with(Protocol::QuicV1))
}
