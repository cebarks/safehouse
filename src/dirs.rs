use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::config::SafehouseConfig;

/// All disk locations safehouse owns or reads.
#[derive(Debug, Clone)]
pub struct SafehouseDirs {
    /// Root safehouse data dir (where safehouse.toml and safehouse.db live)
    pub root: PathBuf,
}

impl SafehouseDirs {
    pub fn from_root(root: PathBuf) -> Self {
        Self { root }
    }

    /// Auto-detect: explicit arg → SAFEHOUSE_DIR env → ~/.local/share/safehouse
    pub fn detect(explicit: Option<&Path>) -> Result<Self> {
        if let Some(p) = explicit {
            return Ok(Self::from_root(p.to_path_buf()));
        }
        if let Ok(env) = std::env::var("SAFEHOUSE_DIR") {
            return Ok(Self::from_root(PathBuf::from(env)));
        }
        let default = dirs_next::data_local_dir()
            .context("cannot resolve local data dir")?
            .join("safehouse");
        Ok(Self::from_root(default))
    }

    pub fn config_path(&self) -> PathBuf {
        self.root.join("safehouse.toml")
    }
    pub fn db_path(&self) -> PathBuf {
        self.root.join("safehouse.db")
    }
    pub fn backups_dir(&self) -> PathBuf {
        self.root.join("backups")
    }
    pub fn run_dir(&self) -> PathBuf {
        self.root.join("run")
    }
    pub fn pid_file(&self) -> PathBuf {
        self.run_dir().join("server.pid")
    }
    pub fn log_dir(&self) -> PathBuf {
        self.root.join("logs")
    }

    pub fn ensure_dirs(&self) -> Result<()> {
        for dir in [
            &self.root,
            &self.backups_dir(),
            &self.run_dir(),
            &self.log_dir(),
        ] {
            std::fs::create_dir_all(dir)
                .with_context(|| format!("cannot create directory {}", dir.display()))?;
        }
        Ok(())
    }

    /// Resolve the server config file path: <zomboid_dir>/Server/<name>.ini
    /// Note: PZ uses capital-S "Server" on Linux.
    pub fn server_ini(&self, config: &SafehouseConfig) -> PathBuf {
        config
            .zomboid_dir()
            .join("Server")
            .join(format!("{}.ini", config.server_name))
    }

    pub fn sandbox_lua(&self, config: &SafehouseConfig) -> PathBuf {
        config
            .zomboid_dir()
            .join("Server")
            .join(format!("{}_SandboxVars.lua", config.server_name))
    }

    pub fn saves_dir(&self, config: &SafehouseConfig) -> PathBuf {
        config
            .zomboid_dir()
            .join("Saves")
            .join("Multiplayer")
            .join(&config.server_name)
    }

    /// Find the most recent PZ log file for this server.
    pub fn latest_log(&self, config: &SafehouseConfig) -> Option<PathBuf> {
        let pattern = config
            .zomboid_dir()
            .join("Server")
            .join(format!("{}_*.txt", config.server_name))
            .to_string_lossy()
            .to_string();
        glob::glob(&pattern)
            .ok()?
            .flatten()
            .max_by_key(|p| p.metadata().ok().and_then(|m| m.modified().ok()))
    }
}
