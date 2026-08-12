use anyhow::{Context, Result};

use crate::config::SafehouseConfig;
use crate::db::Database;
use crate::dirs::SafehouseDirs;

pub struct CliContext {
    pub dirs: SafehouseDirs,
    pub config: SafehouseConfig,
    pub db: Database,
    pub http: reqwest::Client,
}

pub fn resolve_context(cli: &super::Cli) -> Result<CliContext> {
    let dirs = SafehouseDirs::detect(cli.data_dir.as_deref())?;
    dirs.ensure_dirs()?;

    let config_path = cli.config.clone().unwrap_or_else(|| dirs.config_path());
    let config = if config_path.exists() {
        SafehouseConfig::load(&config_path)?
    } else {
        anyhow::bail!(
            "No config found at {}. Run `safehouse setup` first.",
            config_path.display()
        )
    };

    let db = Database::open(&dirs.db_path()).context("failed to open database")?;

    let http = reqwest::Client::builder()
        .user_agent(concat!("safehouse/", env!("SAFEHOUSE_VERSION")))
        .build()?;

    Ok(CliContext {
        dirs,
        config,
        db,
        http,
    })
}
