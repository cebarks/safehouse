use std::time::Duration;

use anyhow::{bail, Context, Result};
use std::process::Stdio;
use tokio::process::Command;

use super::common::CliContext;
use super::ServerAction;
use crate::pz::detect::{find_server_binary, is_server_running, read_pid};
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
    if is_server_running(&ctx.dirs.pid_file()) {
        bail!("Server is already running. Use `safehouse server status` to check.");
    }

    // Acquire exclusive lock on PID file to prevent concurrent starts
    let pid_lock = crate::pz::detect::lock_pid_file(&ctx.dirs.pid_file())
        .context("cannot acquire PID file lock")?;

    let install_dir = &ctx.config.server_install_dir;
    // Verify the PZ install is valid
    let _binary = find_server_binary(install_dir)
        .with_context(|| format!("PZ binary not found in {}", install_dir.display()))?;
    // Use start-server.sh wrapper which sets up LD_LIBRARY_PATH, PATH, and JRE
    let launcher = install_dir.join("start-server.sh");
    if !launcher.exists() {
        bail!("start-server.sh not found in {} — is the PZ dedicated server installed correctly?",
              install_dir.display());
    }

    println!("Starting PZ server '{}'...", ctx.config.server_name);

    // Ensure ~/Zomboid/server/ directory exists for first-run .ini generation
    let server_ini_path = ctx.dirs.server_ini(&ctx.config);
    if let Some(parent) = server_ini_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("cannot create server config dir {}", parent.display()))?;
    }

    // Write RCON settings into server.ini before start (PZ has no -rconpassword CLI flag).
    // If the .ini doesn't exist yet (first run), PZ will generate it — we create a
    // minimal stub so RCON is enabled from the very first boot.
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

    // Capture PZ stdout/stderr to log files so startup failures are diagnosable
    let log_dir = ctx.dirs.log_dir();
    std::fs::create_dir_all(&log_dir)?;
    let stdout_path = log_dir.join("pz-stdout.log");
    let stderr_path = log_dir.join("pz-stderr.log");
    let stdout_file = std::fs::File::create(&stdout_path)
        .with_context(|| format!("cannot create {}", stdout_path.display()))?;
    let stderr_file = std::fs::File::create(&stderr_path)
        .with_context(|| format!("cannot create {}", stderr_path.display()))?;

    let mut cmd = Command::new(&launcher);
    cmd.arg("-servername")
        .arg(&ctx.config.server_name);

    // Pass -adminpassword so PZ doesn't prompt interactively on first run
    if !ctx.config.rcon_password.is_empty() {
        cmd.arg("-adminpassword")
            .arg(&ctx.config.rcon_password);
    }

    let child = cmd
        .current_dir(install_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file))
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
