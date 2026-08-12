use std::time::Duration;

use anyhow::{bail, Context, Result};
use std::process::Stdio;
use tokio::process::Command;

use super::common::CliContext;
use super::ServerAction;
use crate::pz::detect::{find_server_binary, is_server_running, read_pid};

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
    if is_server_running(&ctx.dirs.pid_file()) {
        bail!("Server is already running. Use `safehouse server status` to check.");
    }

    // Acquire exclusive lock on PID file to prevent concurrent starts
    let pid_lock = crate::pz::detect::lock_pid_file(&ctx.dirs.pid_file())
        .context("cannot acquire PID file lock")?;

    let install_dir = &ctx.config.server_install_dir;
    let binary = find_server_binary(install_dir)
        .with_context(|| format!("PZ binary not found in {}", install_dir.display()))?;

    println!("Starting PZ server '{}'...", ctx.config.server_name);

    let child = Command::new(&binary)
        .arg("-servername")
        .arg(&ctx.config.server_name)
        .arg("-rconpassword")
        .arg(&ctx.config.rcon_password)
        .current_dir(install_dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("failed to spawn server process")?;

    let pid = child.id().context("failed to get server PID")?;
    // Write PID to the locked file
    use std::io::Write;
    let mut pid_lock = pid_lock;
    write!(pid_lock, "{}", pid)?;

    // Detach — let the process run independently
    tokio::spawn(async move {
        let _ = child.wait_with_output().await;
    });

    // Wait for RCON to become available (indicates server is ready)
    println!("Waiting for server to become ready (timeout: {timeout_secs}s)...");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_secs);
    loop {
        if tokio::time::Instant::now() > deadline {
            bail!("Server did not respond within {timeout_secs}s — check logs with `safehouse server logs`");
        }
        if let Ok(mut rcon) = crate::pz::rcon::RconClient::connect(
            "127.0.0.1",
            ctx.config.rcon_port,
            &ctx.config.rcon_password,
        ) {
            let _ = rcon.send_command("help");
            println!("Server is ready (PID {pid}).");
            return Ok(());
        }
        tokio::time::sleep(Duration::from_secs(3)).await;
    }
}

pub async fn stop(ctx: &CliContext) -> Result<()> {
    if !is_server_running(&ctx.dirs.pid_file()) {
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
    } else {
        tracing::warn!("Could not connect to RCON; sending SIGTERM directly");
        if let Some(pid) = read_pid(&ctx.dirs.pid_file()) {
            unsafe {
                libc::kill(pid as i32, libc::SIGTERM);
            }
        }
    }

    // Wait for process to exit
    for _ in 0..20 {
        tokio::time::sleep(Duration::from_secs(2)).await;
        if !is_server_running(&ctx.dirs.pid_file()) {
            let _ = std::fs::remove_file(ctx.dirs.pid_file());
            println!("Server stopped.");
            return Ok(());
        }
    }

    // Force kill after 40s
    if let Some(pid) = read_pid(&ctx.dirs.pid_file()) {
        unsafe {
            libc::kill(pid as i32, libc::SIGKILL);
        }
    }
    let _ = std::fs::remove_file(ctx.dirs.pid_file());
    println!("Server force-killed.");
    Ok(())
}

async fn restart(ctx: &CliContext) -> Result<()> {
    stop(ctx).await?;
    tokio::time::sleep(Duration::from_secs(2)).await;
    start(ctx, 60).await
}

async fn logs(ctx: &CliContext, follow: bool, lines: usize) -> Result<()> {
    let log_path = ctx
        .dirs
        .latest_log(&ctx.config)
        .context("No PZ log file found. Has the server been started at least once?")?;

    if follow {
        println!("Following {} (Ctrl+C to stop)...", log_path.display());
        let mut pos = std::fs::metadata(&log_path)?.len();
        loop {
            let meta = std::fs::metadata(&log_path)?;
            if meta.len() > pos {
                use std::io::{Read, Seek, SeekFrom};
                let mut f = std::fs::File::open(&log_path)?;
                f.seek(SeekFrom::Start(pos))?;
                let mut buf = String::new();
                f.read_to_string(&mut buf)?;
                print!("{buf}");
                pos = meta.len();
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    } else {
        let tail = crate::pz::logs::tail_lines(&log_path, lines)?;
        for line in tail {
            println!("{line}");
        }
    }
    #[allow(unreachable_code)]
    Ok(())
}

async fn status(ctx: &CliContext) -> Result<()> {
    let running = is_server_running(&ctx.dirs.pid_file());
    let pid = read_pid(&ctx.dirs.pid_file());

    println!(
        "Server:  {}",
        if running { "🟢 Running" } else { "🔴 Stopped" }
    );
    if let Some(p) = pid {
        println!("PID:     {p}");
    }
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
