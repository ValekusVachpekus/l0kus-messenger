//! Оркестратор: связывает сетевые события libp2p, Olm-крипто и каналы TUI.

use std::collections::HashMap;

use anyhow::Result;
use futures::StreamExt;
use libp2p::{PeerId, swarm::SwarmEvent};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};
use vodozemac::Curve25519PublicKey;
use vodozemac::olm::{OlmMessage, Session};

use crate::crypto::{self, Plain};
use crate::file_transfer::{self, FileSend, IncomingFile};
use crate::identity::Identity;
use crate::message::{AppEvent, Presence, UiCommand};
use crate::net::protocol::{Ack, WireMsg};
use crate::net::swarm;
use crate::net::{Behaviour, BehaviourEvent};

/// Состояние одного пира.
struct PeerState {
    nick: String,
    ed25519_b64: String,
    identity_curve: Option<Curve25519PublicKey>,
    one_time_key: Option<Curve25519PublicKey>,
    session: Option<Session>,
    verified: bool,
    /// Отправили ли мы наш бандл этому пиру.
    bundle_sent: bool,
    /// Сообщили ли мы в TUI о его обнаружении.
    announced: bool,
    /// Очередь исходящих нагрузок, ждущих установления сессии.
    pending_out: Vec<Plain>,
}

impl PeerState {
    fn new() -> Self {
        PeerState {
            nick: String::new(),
            ed25519_b64: String::new(),
            identity_curve: None,
            one_time_key: None,
            session: None,
            verified: false,
            bundle_sent: false,
            announced: false,
            pending_out: Vec::new(),
        }
    }
}

pub struct Engine {
    swarm: libp2p::Swarm<Behaviour>,
    identity: Identity,
    local_peer: PeerId,
    peers: HashMap<PeerId, PeerState>,
    to_ui: mpsc::UnboundedSender<AppEvent>,
    from_ui: mpsc::UnboundedReceiver<UiCommand>,
    /// Незавершённые входящие передачи файлов: (пир, id) -> накопитель.
    incoming_files: HashMap<(PeerId, u64), IncomingFile>,
    next_file_id: u64,
    /// Узел уже запущен (слушает) — защита от повторного `Start`.
    started: bool,
}

impl Engine {
    pub fn new(
        identity: Identity,
        to_ui: mpsc::UnboundedSender<AppEvent>,
        from_ui: mpsc::UnboundedReceiver<UiCommand>,
    ) -> Result<Self> {
        let swarm = swarm::build()?;
        let local_peer = *swarm.local_peer_id();
        Ok(Engine {
            swarm,
            identity,
            local_peer,
            peers: HashMap::new(),
            to_ui,
            from_ui,
            incoming_files: HashMap::new(),
            next_file_id: 1,
            started: false,
        })
    }

    /// Запустить листенер и набрать адреса. Идемпотентно: повторный вызов
    /// игнорируется. Вызывается обработкой `UiCommand::Start`.
    fn start(&mut self, nick: String, port: u16, dials: Vec<String>) {
        if self.started {
            return;
        }
        self.started = true;
        self.identity.set_nick(nick);
        if let Err(e) = swarm::listen(&mut self.swarm, port) {
            self.status(format!("не удалось начать слушать на порту {port}: {e}"));
            return;
        }
        for addr in dials {
            match swarm::parse_dial(&addr) {
                Ok(ma) => {
                    if let Err(e) = self.swarm.dial(ma) {
                        self.status(format!("не удалось набрать {addr}: {e}"));
                    }
                }
                Err(e) => self.status(format!("неверный адрес {addr}: {e}")),
            }
        }
        self.status(format!(
            "ваш fingerprint: {} (ник: {})",
            self.identity.fingerprint(),
            self.identity.nick()
        ));
    }

    pub async fn run(mut self) -> Result<()> {
        loop {
            tokio::select! {
                event = self.swarm.select_next_some() => {
                    self.handle_swarm_event(event);
                }
                cmd = self.from_ui.recv() => {
                    match cmd {
                        Some(UiCommand::Quit) | None => break,
                        Some(cmd) => self.handle_ui_command(cmd),
                    }
                }
            }
        }
        Ok(())
    }

    // --- команды от TUI ------------------------------------------------------

    fn handle_ui_command(&mut self, cmd: UiCommand) {
        match cmd {
            UiCommand::Start { nick, port, dials } => self.start(nick, port, dials),
            UiCommand::SendText { peer, text } => {
                self.queue_out(peer, Plain::Text(text));
            }
            UiCommand::SendFile { peer, path } => match file_transfer::prepare(&path) {
                Ok(FileSend { name, size, chunks }) => {
                    let id = self.next_file_id;
                    self.next_file_id += 1;
                    self.queue_out(
                        peer,
                        Plain::FileOffer {
                            id,
                            name: name.clone(),
                            size,
                        },
                    );
                    for (offset, data) in chunks {
                        self.queue_out(
                            peer,
                            Plain::FileChunk {
                                id,
                                offset,
                                total: size,
                                data,
                            },
                        );
                    }
                    self.emit(AppEvent::FileProgress {
                        peer,
                        name,
                        received: size,
                        total: size,
                        done: None,
                        outgoing: true,
                    });
                }
                Err(e) => self.status(format!("не удалось прочитать файл: {e}")),
            },
            UiCommand::VerifyPeer { peer } => {
                if let Some(p) = self.peers.get_mut(&peer) {
                    p.verified = true;
                    let nick = p.nick.clone();
                    self.emit(AppEvent::Status(format!("пир {nick} помечен доверенным")));
                }
            }
            UiCommand::Dial { addr } => match swarm::parse_dial(&addr) {
                Ok(ma) => {
                    if let Err(e) = self.swarm.dial(ma) {
                        self.status(format!("не удалось набрать {addr}: {e}"));
                    } else {
                        self.status(format!("набираю {addr}…"));
                    }
                }
                Err(e) => self.status(format!("неверный адрес {addr}: {e}")),
            },
            UiCommand::Quit => {}
        }
    }

    // --- сетевые события -----------------------------------------------------

    fn handle_swarm_event(&mut self, event: SwarmEvent<BehaviourEvent>) {
        match event {
            SwarmEvent::NewListenAddr { address, .. } => {
                let full = format!("{address}/p2p/{}", self.local_peer);
                self.emit(AppEvent::Listening { addr: full });
            }
            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                self.peers.entry(peer_id).or_insert_with(PeerState::new);
                self.set_presence(peer_id, Presence::Online);
                self.send_bundle(peer_id);
            }
            SwarmEvent::ConnectionClosed {
                peer_id,
                num_established: 0,
                ..
            } => {
                self.set_presence(peer_id, Presence::Offline);
            }
            SwarmEvent::Behaviour(ev) => self.handle_behaviour_event(ev),
            _ => {}
        }
    }

    fn handle_behaviour_event(&mut self, ev: BehaviourEvent) {
        match ev {
            BehaviourEvent::Mdns(libp2p::mdns::Event::Discovered(list)) => {
                // До нажатия Start (ввод ника на стартовом экране) мы ещё не
                // слушаем и не должны сами набирать пиров: иначе уйдёт KeyBundle
                // с дефолтным ником ОС, и собеседник запомнит неверное имя.
                if !self.started {
                    return;
                }
                for (peer_id, addr) in list {
                    debug!(%peer_id, %addr, "mDNS обнаружил пира");
                    if let Err(e) = self.swarm.dial(addr) {
                        debug!("dial после mDNS не удался: {e}");
                    }
                }
            }
            BehaviourEvent::Rr(libp2p::request_response::Event::Message {
                peer, message, ..
            }) => match message {
                libp2p::request_response::Message::Request {
                    request, channel, ..
                } => {
                    self.handle_wire(peer, request);
                    let _ = self.swarm.behaviour_mut().rr.send_response(channel, Ack::Ok);
                }
                libp2p::request_response::Message::Response { .. } => {}
            },
            BehaviourEvent::Rr(libp2p::request_response::Event::OutboundFailure {
                peer,
                error,
                ..
            }) => {
                warn!(%peer, "ошибка отправки: {error}");
            }
            _ => {}
        }
    }

    // --- обработка wire-сообщений --------------------------------------------

    fn handle_wire(&mut self, peer: PeerId, msg: WireMsg) {
        match msg {
            WireMsg::KeyBundle {
                ed25519,
                curve25519,
                one_time_key,
                nick,
            } => self.on_key_bundle(peer, ed25519, curve25519, one_time_key, nick),
            WireMsg::Encrypted { ty, body } => self.on_encrypted(peer, ty, &body),
        }
    }

    fn on_key_bundle(
        &mut self,
        peer: PeerId,
        ed25519: String,
        curve25519: String,
        one_time_key: String,
        nick: String,
    ) {
        let identity_curve = Curve25519PublicKey::from_base64(&curve25519).ok();
        let otk = Curve25519PublicKey::from_base64(&one_time_key).ok();
        let p = self.peers.entry(peer).or_insert_with(PeerState::new);
        p.nick = nick.clone();
        p.ed25519_b64 = ed25519.clone();
        p.identity_curve = identity_curve;
        p.one_time_key = otk;
        let fingerprint = crate::identity::fingerprint_of(&ed25519);
        if !p.announced {
            p.announced = true;
            self.emit(AppEvent::PeerDiscovered {
                peer,
                nick,
                fingerprint,
            });
        }
        // Возможно, пора инициировать сессию.
        self.maybe_initiate(peer);
    }

    fn on_encrypted(&mut self, peer: PeerId, ty: u8, body: &[u8]) {
        // Расшифровка существующей сессией либо создание inbound из PreKey.
        let plain = {
            let has_session = self
                .peers
                .get(&peer)
                .map(|p| p.session.is_some())
                .unwrap_or(false);
            if has_session {
                let p = self.peers.get_mut(&peer).unwrap();
                let session = p.session.as_mut().unwrap();
                match crypto::open(session, ty, body) {
                    Ok(plain) => Some(plain),
                    Err(e) => {
                        warn!(%peer, "расшифровка не удалась: {e}");
                        None
                    }
                }
            } else {
                self.try_inbound(peer, ty, body)
            }
        };
        if let Some(plain) = plain {
            self.on_plain(peer, plain);
        }
    }

    /// Создать inbound-сессию из PreKey-сообщения и вернуть первую нагрузку.
    fn try_inbound(&mut self, peer: PeerId, ty: u8, body: &[u8]) -> Option<Plain> {
        let msg = crypto::parse_message(ty, body).ok()?;
        let prekey = match msg {
            OlmMessage::PreKey(pk) => pk,
            OlmMessage::Normal(_) => {
                warn!(%peer, "получено Normal-сообщение без сессии");
                return None;
            }
        };
        let their_identity = self.peers.get(&peer).and_then(|p| p.identity_curve)?;
        match self.identity.create_inbound(their_identity, &prekey) {
            Ok((session, plaintext)) => {
                if let Some(p) = self.peers.get_mut(&peer) {
                    p.session = Some(session);
                }
                self.flush_pending(peer);
                rmp_serde::from_slice::<Plain>(&plaintext).ok()
            }
            Err(e) => {
                warn!(%peer, "inbound-сессия не создана: {e}");
                None
            }
        }
    }

    fn on_plain(&mut self, peer: PeerId, plain: Plain) {
        match plain {
            Plain::Hello { nick } => {
                if let Some(p) = self.peers.get_mut(&peer)
                    && !nick.is_empty()
                {
                    p.nick = nick;
                }
                debug!(%peer, "получен Hello");
            }
            Plain::Text(text) => {
                self.emit(AppEvent::MessageReceived { peer, text });
            }
            Plain::FileOffer { id, name, size } => {
                let file = self.incoming_files.entry((peer, id)).or_default();
                file.set_name(name.clone());
                file.set_total(size);
                self.finalize_or_progress(peer, id);
            }
            Plain::FileChunk {
                id,
                offset,
                total,
                data,
            } => self.on_file_chunk(peer, id, offset, total, &data),
        }
    }

    fn on_file_chunk(&mut self, peer: PeerId, id: u64, offset: u64, total: u64, data: &[u8]) {
        let file = self.incoming_files.entry((peer, id)).or_default();
        file.set_total(total);
        file.push(offset, data);
        self.finalize_or_progress(peer, id);
    }

    /// Завершить передачу при полноте, иначе сообщить о прогрессе.
    fn finalize_or_progress(&mut self, peer: PeerId, id: u64) {
        let key = (peer, id);
        let Some(file) = self.incoming_files.get(&key) else {
            return;
        };
        let received = file.received();
        let total = file.total();
        let name = file.name();
        if file.is_complete() {
            let result = file.finish();
            self.incoming_files.remove(&key);
            match result {
                Ok(path) => self.emit(AppEvent::FileProgress {
                    peer,
                    name,
                    received,
                    total,
                    done: Some(path),
                    outgoing: false,
                }),
                Err(e) => self.status(format!("сохранение файла не удалось: {e}")),
            }
        } else {
            self.emit(AppEvent::FileProgress {
                peer,
                name,
                received,
                total,
                done: None,
                outgoing: false,
            });
        }
    }

    // --- установление сессии и отправка --------------------------------------

    /// Отправить наш ключевой бандл пиру (один раз).
    fn send_bundle(&mut self, peer: PeerId) {
        let already = self
            .peers
            .get(&peer)
            .map(|p| p.bundle_sent)
            .unwrap_or(false);
        if already {
            return;
        }
        let otk = self.identity.take_one_time_key();
        let bundle = WireMsg::KeyBundle {
            ed25519: self.identity.identity_ed_base64(),
            curve25519: self.identity.identity_curve().to_base64(),
            one_time_key: otk.to_base64(),
            nick: self.identity.nick().to_string(),
        };
        self.swarm.behaviour_mut().rr.send_request(&peer, bundle);
        self.peers
            .entry(peer)
            .or_insert_with(PeerState::new)
            .bundle_sent = true;
    }

    /// Если мы инициатор (меньший PeerId) и есть бандл пира — создать сессию.
    fn maybe_initiate(&mut self, peer: PeerId) {
        let we_initiate = self.local_peer < peer;
        let p = match self.peers.get(&peer) {
            Some(p) => p,
            None => return,
        };
        if !we_initiate || p.session.is_some() {
            return;
        }
        let (Some(identity_curve), Some(otk)) = (p.identity_curve, p.one_time_key) else {
            return;
        };
        match self.identity.create_outbound(identity_curve, otk) {
            Ok(session) => {
                if let Some(p) = self.peers.get_mut(&peer) {
                    p.session = Some(session);
                }
                // Первый (PreKey) пакет — приветствие, инициирует сессию у пира.
                let hello = Plain::Hello {
                    nick: self.identity.nick().to_string(),
                };
                self.send_plain(peer, &hello);
                self.flush_pending(peer);
            }
            Err(e) => warn!(%peer, "outbound-сессия не создана: {e}"),
        }
    }

    /// Поставить нагрузку в очередь и попытаться отправить.
    fn queue_out(&mut self, peer: PeerId, plain: Plain) {
        let has_session = self
            .peers
            .get(&peer)
            .map(|p| p.session.is_some())
            .unwrap_or(false);
        if has_session {
            self.send_plain(peer, &plain);
        } else {
            self.peers
                .entry(peer)
                .or_insert_with(PeerState::new)
                .pending_out
                .push(plain);
            // Возможно, мы можем стать инициатором прямо сейчас.
            self.maybe_initiate(peer);
        }
    }

    fn flush_pending(&mut self, peer: PeerId) {
        let pending: Vec<Plain> = self
            .peers
            .get_mut(&peer)
            .map(|p| std::mem::take(&mut p.pending_out))
            .unwrap_or_default();
        for plain in pending {
            self.send_plain(peer, &plain);
        }
    }

    /// Зашифровать и отправить нагрузку (требует готовой сессии).
    fn send_plain(&mut self, peer: PeerId, plain: &Plain) {
        let Some(p) = self.peers.get_mut(&peer) else {
            return;
        };
        let Some(session) = p.session.as_mut() else {
            warn!(%peer, "нет сессии для отправки");
            return;
        };
        match crypto::seal(session, plain) {
            Ok((ty, body)) => {
                let msg = WireMsg::Encrypted { ty, body };
                self.swarm.behaviour_mut().rr.send_request(&peer, msg);
            }
            Err(e) => warn!(%peer, "шифрование не удалось: {e}"),
        }
    }

    // --- помощники -----------------------------------------------------------

    fn set_presence(&mut self, peer: PeerId, presence: Presence) {
        self.peers.entry(peer).or_insert_with(PeerState::new);
        self.emit(AppEvent::PeerPresence { peer, presence });
    }

    fn status(&self, msg: String) {
        info!("{msg}");
        self.emit(AppEvent::Status(msg));
    }

    fn emit(&self, ev: AppEvent) {
        let _ = self.to_ui.send(ev);
    }
}
