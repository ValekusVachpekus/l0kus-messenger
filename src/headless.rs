//! Безголовый режим: построчный stdin → команды, события → stdout.
//!
//! Полезен для скриптов и end-to-end тестов (TUI требует TTY). Формат вывода
//! машиночитаемый: `LISTEN`, `PEER`, `PRESENCE`, `MSG`, `FILE`, `STATUS`.

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use libp2p::PeerId;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc;

use crate::message::{AppEvent, Presence, UiCommand};

pub async fn run(
    to_engine: mpsc::UnboundedSender<UiCommand>,
    mut from_engine: mpsc::UnboundedReceiver<AppEvent>,
    nick: String,
    fingerprint: String,
) -> Result<()> {
    println!("READY {nick} {fingerprint}");

    let mut peers: Vec<PeerId> = Vec::new();
    let mut names: HashMap<PeerId, String> = HashMap::new();
    let stdin = BufReader::new(tokio::io::stdin());
    let mut lines = stdin.lines();

    loop {
        tokio::select! {
            ev = from_engine.recv() => {
                let Some(ev) = ev else { break };
                handle_event(ev, &mut peers, &mut names);
            }
            line = lines.next_line() => {
                let line = match line { Ok(Some(l)) => l, _ => break };
                if !handle_line(line.trim(), &to_engine, &peers) {
                    break;
                }
            }
        }
    }
    let _ = to_engine.send(UiCommand::Quit);
    Ok(())
}

fn handle_event(ev: AppEvent, peers: &mut Vec<PeerId>, names: &mut HashMap<PeerId, String>) {
    match ev {
        AppEvent::Listening { addr } => println!("LISTEN {addr}"),
        AppEvent::PeerDiscovered {
            peer,
            nick,
            fingerprint,
        } => {
            if !peers.contains(&peer) {
                peers.push(peer);
            }
            names.insert(peer, nick.clone());
            println!("PEER {peer} {nick} {fingerprint}");
        }
        AppEvent::PeerPresence { peer, presence } => {
            let s = match presence {
                Presence::Online => "online",
                Presence::Offline => "offline",
            };
            println!("PRESENCE {peer} {s}");
        }
        AppEvent::MessageReceived { peer, text } => println!("MSG {peer} {text}"),
        AppEvent::FileProgress {
            peer,
            name,
            received,
            total,
            done,
            outgoing,
        } => {
            let state = if outgoing {
                "sent".to_string()
            } else if let Some(path) = done {
                format!("recv-done {}", path.display())
            } else {
                format!("recv {received}/{total}")
            };
            println!("FILE {peer} {state} {name}");
        }
        AppEvent::Status(msg) => println!("STATUS {msg}"),
    }
}

/// Возвращает false, если пора завершаться.
fn handle_line(line: &str, to_engine: &mpsc::UnboundedSender<UiCommand>, peers: &[PeerId]) -> bool {
    if line.is_empty() {
        return true;
    }
    if line == "/quit" {
        return false;
    }
    if let Some(addr) = line.strip_prefix("/dial ") {
        let _ = to_engine.send(UiCommand::Dial {
            addr: addr.trim().to_string(),
        });
        return true;
    }
    if line == "/refresh" {
        let _ = to_engine.send(UiCommand::RefreshDiscovery);
        return true;
    }
    let target = peers.first().copied();
    let Some(peer) = target else {
        println!("STATUS нет пиров для отправки");
        return true;
    };
    if line == "/verify" {
        let _ = to_engine.send(UiCommand::VerifyPeer { peer });
    } else if let Some(path) = line.strip_prefix("/file ") {
        let _ = to_engine.send(UiCommand::SendFile {
            peer,
            path: PathBuf::from(path.trim()),
        });
    } else {
        let _ = to_engine.send(UiCommand::SendText {
            peer,
            text: line.to_string(),
        });
    }
    true
}
