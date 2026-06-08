//! p2p-chat — локальный P2P мессенджер со сквозным шифрованием.

mod config;
mod crypto;
mod engine;
mod file_transfer;
mod headless;
mod identity;
mod message;
mod net;
mod settings;
mod ui;

use anyhow::Result;
use clap::Parser;
use tokio::sync::mpsc;
use tracing_subscriber::EnvFilter;

use config::Config;
use engine::Engine;
use identity::Identity;
use message::UiCommand;
use settings::Settings;
use ui::StartupConfig;

#[tokio::main]
async fn main() -> Result<()> {
    let config = Config::parse();
    // Флаг --data-dir переопределяет каталог данных (до чтения идентичности/логов).
    if let Some(dir) = &config.data_dir {
        // SAFETY: однопоточный старт до запуска tokio-задач и любых чтений env.
        unsafe {
            std::env::set_var("P2P_CHAT_DATA_DIR", dir);
        }
    }
    let _guard = init_logging()?;

    let interactive = config.is_interactive();
    let saved = Settings::load();

    let identity = Identity::load_or_create(config.nickname())?;
    let me_fingerprint = identity.fingerprint();

    // Каналы между TUI и engine.
    let (to_ui, from_engine) = mpsc::unbounded_channel();
    let (to_engine, from_ui) = mpsc::unbounded_channel();

    let engine = Engine::new(identity, to_ui, from_ui)?;
    let engine_handle = tokio::spawn(async move {
        if let Err(e) = engine.run().await {
            tracing::error!("engine завершился с ошибкой: {e}");
        }
    });

    // Стартовые значения для формы/немедленного запуска: флаги имеют приоритет,
    // иначе сохранённые настройки, иначе дефолты.
    let nick = config
        .nick
        .clone()
        .or(saved.nick.clone())
        .unwrap_or_else(config::default_nick);
    let port = config.port.unwrap_or(saved.port);
    let dials = if config.dial.is_empty() {
        saved.dial.clone()
    } else {
        config.dial.clone()
    };

    // Неинтерактивный запуск (headless или флаги) — стартуем сразу.
    if !interactive {
        let _ = to_engine.send(UiCommand::Start {
            nick: nick.clone(),
            port,
            dials: dials.clone(),
        });
    }

    // TUI (или безголовый режим) работает в основном контексте; выход завершает.
    let result = if config.headless {
        headless::run(to_engine, from_engine, nick, me_fingerprint).await
    } else {
        let startup = StartupConfig {
            interactive,
            nick,
            port,
            dial: dials.first().cloned().unwrap_or_default(),
            remember: saved.remember,
        };
        ui::run(to_engine, from_engine, me_fingerprint, startup).await
    };

    engine_handle.abort();
    result
}

/// Логи пишутся в файл (а не в TUI), уровень управляется `RUST_LOG`.
fn init_logging() -> Result<tracing_appender::non_blocking::WorkerGuard> {
    let dir = identity::data_dir()?;
    std::fs::create_dir_all(&dir).ok();
    let file_appender = tracing_appender::rolling::never(&dir, "p2p-chat.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,libp2p_mdns=warn")),
        )
        .with_writer(non_blocking)
        .with_ansi(false)
        .init();
    Ok(guard)
}
