use std::time::Duration;

use anyhow::{bail, Context, Result};

use super::common::CliContext;
use super::ServerAction;
use crate::container;
use crate::pz::ini::IniEditor;

pub async fn run(action: &ServerAction, ctx: &CliContext) -> Result<()> {
    match action {
        ServerAction::Start { timeout } => start(ctx, *timeout).await,
        ServerAction::Stop => stop(ctx).await,
        ServerAction::Restart => restart(ctx).await,
        ServerAction::Logs { follow, lines } => logs(ctx, *follow, *lines).await,
        ServerAction::Status => status(ctx).await,
    }
}

async fn start(ctx: &CliContext, timeout_secs: u64) -> Result<()> {
    let docker = container::connect().await?;
    let image = container::ensure_image(&docker).await?;

    if container::is_running(&docker).await {
        bail!("Server is already running. Use `safehouse server status` to check.");
    }

    println!("Starting PZ server '{}'...", ctx.config.server_name);

    // Ensure ~/Zomboid/Server/ directory exists for first-run .ini generation
    let server_ini_path = ctx.dirs.server_ini(&ctx.config);
    if let Some(parent) = server_ini_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("cannot create server config dir {}", parent.display()))?;
    }

    // Write RCON settings into server.ini before start.
    // If the .ini doesn't exist yet (first run), we create a minimal stub
    // so RCON is configured from the very first boot.
    if !ctx.config.rcon_password.is_empty() {
        if server_ini_path.exists() {
            let mut ini = IniEditor::load(&server_ini_path)
                .with_context(|| format!("cannot read {}", server_ini_path.display()))?;
            ini.set("RCONPassword", &ctx.config.rcon_password);
            ini.set("RCONPort", &ctx.config.rcon_port.to_string());
            ini.save(&server_ini_path)?;
        } else {
            let content = format!(
                "RCONPassword={}\nRCONPort={}\n",
                ctx.config.rcon_password, ctx.config.rcon_port
            );
            std::fs::write(&server_ini_path, content)
                .with_context(|| format!("cannot write {}", server_ini_path.display()))?;
        }
        tracing::info!("Wrote RCON settings to {}", server_ini_path.display());
    }

    // Start the container
    container::create_and_start(&docker, &ctx.config, &image).await?;

    // Wait for RCON to become available (indicates server is ready)
    println!("Waiting for server to become ready (timeout: {timeout_secs}s)...");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_secs);
    loop {
        if tokio::time::Instant::now() > deadline {
            bail!("Server did not respond within {timeout_secs}s — check logs with `safehouse server logs`");
        }

        // Check the container is still alive (didn't crash on startup)
        if !container::is_running(&docker).await {
            bail!("Server container exited unexpectedly — check logs with `safehouse server logs`");
        }

        if let Ok(mut rcon) = crate::pz::rcon::RconClient::connect(
            "127.0.0.1",
            ctx.config.rcon_port,
            &ctx.config.rcon_password,
        ) {
            let _ = rcon.send_command("help");
            println!("Server is ready.");
            return Ok(());
        }
        tokio::time::sleep(Duration::from_secs(3)).await;
    }
}

pub async fn stop(ctx: &CliContext) -> Result<()> {
    let docker = container::connect().await?;

    if !container::is_running(&docker).await {
        println!("Server is not running.");
        return Ok(());
    }

    // Graceful: RCON save then quit
    println!("Saving world and stopping server...");
    if let Ok(mut rcon) = crate::pz::rcon::RconClient::connect(
        "127.0.0.1",
        ctx.config.rcon_port,
        &ctx.config.rcon_password,
    ) {
        let _ = rcon.send_command("save");
        tokio::time::sleep(Duration::from_secs(3)).await;
        let _ = rcon.send_command("quit");

        // Wait briefly for RCON quit to take effect
        for _ in 0..10 {
            tokio::time::sleep(Duration::from_secs(2)).await;
            if !container::is_running(&docker).await {
                container::remove(&docker).await?;
                println!("Server stopped.");
                return Ok(());
            }
        }
    }

    // Fallback: container stop (sends SIGTERM to PID 1 = Java process)
    tracing::warn!("RCON quit did not stop server; stopping container directly");
    container::stop(&docker, 30).await?;
    container::remove(&docker).await?;
    println!("Server stopped.");
    Ok(())
}

async fn restart(ctx: &CliContext) -> Result<()> {
    stop(ctx).await?;
    tokio::time::sleep(Duration::from_secs(2)).await;
    start(ctx, 60).await
}

async fn logs(_ctx: &CliContext, follow: bool, lines: usize) -> Result<()> {
    let docker = container::connect().await?;

    // Check container exists (running or stopped)
    match docker
        .inspect_container(container::CONTAINER_NAME, None)
        .await
    {
        Ok(_) => {}
        Err(_) => bail!("No server container found. Has the server been started at least once?"),
    }

    if follow {
        println!("Following server logs (Ctrl+C to stop)...");
    }

    container::stream_logs(&docker, follow, lines).await?;

    #[allow(unreachable_code)]
    Ok(())
}

async fn status(ctx: &CliContext) -> Result<()> {
    let docker = container::connect().await?;
    let running = container::is_running(&docker).await;

    println!(
        "Server:  {}",
        if running { "🟢 Running" } else { "🔴 Stopped" }
    );
    println!("Name:    {}", ctx.config.server_name);
    println!("Install: {}", ctx.config.server_install_dir.display());

    if running {
        if let Ok(mut rcon) = crate::pz::rcon::RconClient::connect(
            "127.0.0.1",
            ctx.config.rcon_port,
            &ctx.config.rcon_password,
        ) {
            let players = rcon
                .send_command("players")
                .unwrap_or_else(|_| "?".to_string());
            println!("Players: {players}");
        }
    }
    Ok(())
}
