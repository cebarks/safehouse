use anyhow::{bail, Context, Result};

use super::common::CliContext;
use super::BackupAction;
use crate::backup::{create_snapshot, list_snapshots, prune_snapshots, restore_snapshot};
use crate::pz::rcon::RconClient;

pub async fn run(action: &BackupAction, ctx: &CliContext) -> Result<()> {
    match action {
        BackupAction::Create { label } => create(ctx, label.as_deref()).await,
        BackupAction::List => list(ctx),
        BackupAction::Restore { filename } => restore(ctx, filename).await,
        BackupAction::Prune { min_keep } => prune(ctx, *min_keep),
    }
}

async fn create(ctx: &CliContext, label: Option<&str>) -> Result<()> {
    // RCON save first if configured
    if ctx.config.backup_rcon_save {
        if let Ok(mut rcon) =
            RconClient::connect("127.0.0.1", ctx.config.rcon_port, &ctx.config.rcon_password)
        {
            println!("Saving world before backup...");
            let _ = rcon.send_command("save");
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        }
    }

    let saves_dir = ctx.dirs.saves_dir(&ctx.config);
    if !saves_dir.exists() {
        bail!("World save directory not found: {}", saves_dir.display());
    }

    let snap = create_snapshot(
        &saves_dir,
        &ctx.dirs.backups_dir(),
        &ctx.config.server_name,
        label,
    )?;
    let size = snap.metadata().map(|m| m.len()).unwrap_or(0);
    let filename = snap
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    ctx.db.record_backup(
        &filename,
        label,
        Some(size as i64),
        &ctx.config.server_name,
        "cli",
    )?;
    println!(
        "Backup created: {filename} ({:.1} MB)",
        size as f64 / 1_048_576.0
    );
    Ok(())
}

fn list(ctx: &CliContext) -> Result<()> {
    let snaps = list_snapshots(&ctx.dirs.backups_dir())?;
    if snaps.is_empty() {
        println!("No backups found.");
        return Ok(());
    }
    for snap in snaps {
        let size = snap.metadata().map(|m| m.len()).unwrap_or(0);
        println!(
            "{:<50} {:.1} MB",
            snap.file_name().unwrap_or_default().to_string_lossy(),
            size as f64 / 1_048_576.0
        );
    }
    Ok(())
}

async fn restore(ctx: &CliContext, filename: &str) -> Result<()> {
    let snap_path = ctx.dirs.backups_dir().join(filename);
    if !snap_path.exists() {
        bail!("Backup not found: {filename}");
    }
    // Ensure server is stopped first
    if crate::pz::detect::is_server_running(&ctx.dirs.pid_file()) {
        println!("Stopping server before restore...");
        crate::cli::server::stop(ctx).await?;
    }
    let saves_dir = ctx.dirs.saves_dir(&ctx.config);
    restore_snapshot(&snap_path, &saves_dir).context("restore failed")?;
    println!("Restored from {filename}. Start the server when ready.");
    Ok(())
}

fn prune(ctx: &CliContext, min_keep: usize) -> Result<()> {
    let pruned = prune_snapshots(
        &ctx.dirs.backups_dir(),
        ctx.config.backup_retention_days,
        min_keep,
    )?;
    if pruned.is_empty() {
        println!("Nothing to prune.");
    } else {
        for p in &pruned {
            let fname = p.file_name().unwrap_or_default().to_string_lossy();
            println!("Deleted: {fname}");
            let _ = ctx.db.delete_backup_record(&fname);
        }
        println!("Pruned {} backup(s).", pruned.len());
    }
    Ok(())
}
