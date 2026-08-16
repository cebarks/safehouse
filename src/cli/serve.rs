use anyhow::{Context, Result};

use crate::config::SafehouseConfig;
use crate::db::Database;
use crate::dirs::SafehouseDirs;

pub async fn run(bind: Option<&str>, port: Option<u16>, cli: &super::Cli) -> Result<()> {
    let dirs = SafehouseDirs::detect(cli.data_dir.as_deref())?;
    dirs.ensure_dirs()?;

    let config_path = dirs.config_path();
    let mut config = SafehouseConfig::load(&config_path)?;

    if let Some(b) = bind {
        config.web_bind = b.to_string();
    }
    if let Some(p) = port {
        config.web_port = p;
    }

    // Ensure admin user exists
    let db = Database::open(&dirs.db_path()).context("failed to open database")?;
    if !db.user_exists("admin")? {
        let password = rpassword::prompt_password("Set admin password: ")?;
        db.create_user("admin", &password)?;
        println!("Admin user created.");
    }

    config.ensure_session_secret();
    config.save(&config_path)?;

    let web_bind = config.web_bind.clone();
    let web_port = config.web_port;

    // Clone config for log watcher before moving config into run_server
    let log_watcher_config = config.clone();
    let log_watcher_dirs = dirs.clone();

    // Connect to podman/docker for container status queries
    let docker = crate::container::connect().await?;

    // Start web server (returns a handle for graceful shutdown)
    let server_handle = crate::web::run_server(&web_bind, web_port, config, dirs, db, docker).await?;

    // Spawn background log watcher for Discord notifications
    let http_client = reqwest::Client::new();
    tokio::spawn(async move {
        let mut last_pos: u64 = 0;
        let mut last_log_path: Option<std::path::PathBuf> = None;
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            let Some(log_path) = log_watcher_dirs.latest_log(&log_watcher_config) else {
                continue;
            };
            // Handle log file rotation: when PZ creates a new log file after restart,
            // seek to current end to avoid replaying historical events as Discord spam.
            if last_log_path.as_ref() != Some(&log_path) {
                last_pos = std::fs::metadata(&log_path)
                    .map(|m| m.len())
                    .unwrap_or(0);
                last_log_path = Some(log_path.clone());
                continue; // skip this tick — start watching from here on next iteration
            }
            let Ok(meta) = std::fs::metadata(&log_path) else {
                continue;
            };
            if meta.len() <= last_pos {
                continue;
            }

            use std::io::{Read, Seek, SeekFrom};
            let Ok(mut f) = std::fs::File::open(&log_path) else {
                continue;
            };
            let _ = f.seek(SeekFrom::Start(last_pos));
            let mut buf = String::new();
            let _ = f.read_to_string(&mut buf);
            last_pos = meta.len();

            for line in buf.lines() {
                if let Some(event) = crate::pz::logs::parse_log_line(line) {
                    let notify_event = match event {
                        crate::pz::logs::PlayerEvent::Connected { ref name } => {
                            Some(crate::notify::NotifyEvent::PlayerJoined(name.clone()))
                        }
                        crate::pz::logs::PlayerEvent::Disconnected { ref name } => {
                            Some(crate::notify::NotifyEvent::PlayerLeft(name.clone()))
                        }
                    };
                    if let Some(ev) = notify_event {
                        let _ = crate::notify::notify(
                            &http_client,
                            log_watcher_config.discord_webhook_url.as_deref(),
                            &log_watcher_config.server_name,
                            ev,
                        )
                        .await;
                    }
                }
            }
        }
    });

    // Signal handling: graceful shutdown on SIGTERM/SIGINT
    #[allow(clippy::unwrap_used)] // signal registration panics are unrecoverable
    let mut sigterm =
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).unwrap();

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("Received SIGINT, shutting down...");
        }
        _ = sigterm.recv() => {
            tracing::info!("Received SIGTERM, shutting down...");
        }
    }

    server_handle.stop(true).await;
    Ok(())
}
