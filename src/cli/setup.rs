use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::config::SafehouseConfig;
use crate::container;
use crate::dirs::SafehouseDirs;

pub async fn run(
    install_dir: Option<&Path>,
    admin_password: Option<&str>,
    data_dir: Option<&Path>,
) -> Result<()> {
    println!("=== Safehouse Setup ===");
    // Default to CWD for both data dir and install dir so
    // `safehouse setup` in a project directory keeps everything local.
    let cwd = std::env::current_dir().context("cannot determine working directory")?;
    let dirs = SafehouseDirs::detect(data_dir.or(Some(cwd.as_path())))?;
    dirs.ensure_dirs()?;

    let install = install_dir
        .map(PathBuf::from)
        .unwrap_or_else(|| cwd.join("pzserver"));

    // Canonicalize so the container bind-mount gets an absolute path
    std::fs::create_dir_all(&install)?;
    let install = install
        .canonicalize()
        .context("failed to resolve install directory")?;

    println!("PZ install directory: {}", install.display());
    println!("Safehouse data dir:   {}", dirs.root.display());

    // Download PZ via SteamCMD inside the container
    let docker = container::connect().await?;
    container::ensure_image(&docker).await?;

    let mut cfg = SafehouseConfig::default();
    cfg.server_install_dir = install;

    // Keep zomboid data inside the data dir (co-located with config + backups)
    let zomboid_dir = dirs.root.join("zomboid");
    std::fs::create_dir_all(&zomboid_dir)?;
    cfg.zomboid_data_dir = Some(
        zomboid_dir
            .canonicalize()
            .context("failed to resolve zomboid data directory")?,
    );

    container::run_steamcmd_install(&docker, &cfg).await?;

    cfg.rcon_password = admin_password.unwrap_or("changeme").to_string();

    let config_path = dirs.config_path();
    cfg.save(&config_path)?;

    println!("\nSetup complete!");
    println!(
        "Edit {} to configure RCON password, server name, etc.",
        config_path.display()
    );
    println!("Then run: safehouse server start");
    Ok(())
}
