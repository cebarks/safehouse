use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::config::SafehouseConfig;
use crate::dirs::SafehouseDirs;

pub async fn run(install_dir: Option<&Path>, admin_password: Option<&str>) -> Result<()> {
    println!("=== Safehouse Setup ===");
    let dirs = SafehouseDirs::detect(None)?;
    dirs.ensure_dirs()?;

    let install = install_dir
        .map(PathBuf::from)
        .unwrap_or_else(|| dirs_next::home_dir().unwrap_or_default().join("pzserver"));

    println!("PZ install directory: {}", install.display());
    println!("Safehouse data dir:   {}", dirs.root.display());

    // Download PZ via SteamCMD
    install_pz_steamcmd(&install).await?;

    let _password = admin_password.unwrap_or("changeme");
    let mut cfg = SafehouseConfig::default();
    cfg.server_install_dir = install;

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

async fn install_pz_steamcmd(install_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(install_dir)?;
    println!("Installing Project Zomboid dedicated server via SteamCMD...");

    // Check steamcmd is available
    let steamcmd = which_steamcmd().context(
        "steamcmd not found. Install it: https://developer.valvesoftware.com/wiki/SteamCMD",
    )?;

    let status = tokio::process::Command::new(&steamcmd)
        .args([
            "+force_install_dir",
            &install_dir.to_string_lossy(),
            "+login",
            "anonymous",
            "+app_update",
            "380870",
            "validate",
            "+quit",
        ])
        .status()
        .await
        .context("steamcmd failed")?;

    if !status.success() {
        anyhow::bail!("steamcmd exited with status: {status}");
    }
    println!("PZ server installed.");
    Ok(())
}

fn which_steamcmd() -> Option<PathBuf> {
    for candidate in ["/usr/games/steamcmd", "/usr/bin/steamcmd", "steamcmd"] {
        let p = PathBuf::from(candidate);
        if p.exists() {
            return Some(p);
        }
    }
    // Try PATH
    std::process::Command::new("which")
        .arg("steamcmd")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| PathBuf::from(String::from_utf8_lossy(&o.stdout).trim().to_string()))
}
