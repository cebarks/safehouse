use std::path::PathBuf;

use clap::{Parser, Subcommand};

pub mod backup;
pub mod common;
pub mod config;
pub mod console;
pub mod mods;
pub mod serve;
pub mod server;
pub mod setup;
pub mod webhook;

#[derive(Parser)]
#[command(
    name = "safehouse",
    version = env!("SAFEHOUSE_VERSION"),
    about = "Project Zomboid dedicated server manager"
)]
pub struct Cli {
    /// Safehouse data directory (default: ~/.local/share/safehouse)
    #[arg(long, global = true)]
    pub data_dir: Option<PathBuf>,

    /// Config file path override
    #[arg(long, global = true)]
    pub config: Option<PathBuf>,

    /// Increase verbosity (-v debug, -vv trace)
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Initialize safehouse and install the PZ dedicated server
    Setup {
        /// Where to install the PZ server (default: ~/pzserver)
        #[arg(long)]
        install_dir: Option<PathBuf>,
        /// Admin password for the PZ server
        #[arg(long)]
        admin_password: Option<String>,
    },

    /// Manage the PZ server process
    Server {
        #[command(subcommand)]
        action: ServerAction,
    },

    /// Edit server configuration files
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Manage Steam Workshop mods
    Mods {
        #[command(subcommand)]
        action: ModAction,
    },

    /// Manage world backups
    Backup {
        #[command(subcommand)]
        action: BackupAction,
    },

    /// Send RCON admin commands
    Console {
        #[command(subcommand)]
        action: ConsoleAction,
    },

    /// Configure Discord webhook notifications
    Webhook {
        /// Discord webhook URL
        #[arg(long)]
        url: Option<String>,
        /// Send a test notification
        #[arg(long)]
        test: bool,
    },

    /// Start the web management UI
    Serve {
        #[arg(long)]
        bind: Option<String>,
        #[arg(long)]
        port: Option<u16>,
    },
}

#[derive(Subcommand)]
pub enum ServerAction {
    /// Start the server
    Start {
        #[arg(long, default_value = "60")]
        timeout: u64,
    },
    /// Stop the server gracefully (RCON save + shutdown)
    Stop,
    /// Restart the server
    Restart,
    /// Stream server logs to stdout
    Logs {
        #[arg(short, long)]
        follow: bool,
        #[arg(long, default_value = "100")]
        lines: usize,
    },
    /// Show server status (running, player count, uptime)
    Status,
}

#[derive(Subcommand)]
pub enum ConfigAction {
    /// Show all current server.ini values
    Show,
    /// Set a key in server.ini
    Set { key: String, value: String },
    /// Edit SandboxVars.lua
    Sandbox {
        #[command(subcommand)]
        action: SandboxAction,
    },
    /// Manage named config presets
    Preset {
        #[command(subcommand)]
        action: PresetAction,
    },
}

#[derive(Subcommand)]
pub enum SandboxAction {
    /// Show SandboxVars.lua contents
    Show,
    /// Set a key in SandboxVars.lua (supports dotted keys like Zombies.Speed)
    Set { key: String, value: String },
}

#[derive(Subcommand)]
pub enum PresetAction {
    /// List saved presets
    List,
    /// Save current mod list as a named preset
    Save { name: String },
    /// Apply a saved preset to server.ini
    Apply { name: String },
}

#[derive(Subcommand)]
pub enum ModAction {
    /// List installed Workshop mods
    List,
    /// Add a mod by Workshop ID
    Add {
        workshop_id: String,
        /// Internal mod folder name (shown in the mod's README or on Workshop page)
        mod_name: String,
    },
    /// Remove a mod by Workshop ID
    Remove { workshop_id: String },
    /// Fetch and display Workshop metadata for an ID
    Info { workshop_id: String },
    /// Sync mods from a Steam Workshop collection
    Sync {
        /// Collection ID or full Steam Workshop URL (overrides steam_collection_id in config)
        collection: Option<String>,
    },
    /// Manage named mod collection profiles
    Profile {
        #[command(subcommand)]
        action: ProfileAction,
    },
    /// Fix case-sensitivity issues by creating lowercase symlinks
    FixCase,
}

#[derive(Subcommand)]
pub enum ProfileAction {
    /// List saved profiles
    List,
    /// Save current mod list as a named profile
    Save { name: String },
    /// Load a saved profile into server.ini
    Load { name: String },
}

#[derive(Subcommand)]
pub enum BackupAction {
    /// Create a snapshot of the world save + configs
    Create {
        #[arg(long)]
        label: Option<String>,
    },
    /// List available snapshots
    List,
    /// Restore a snapshot (stops server first)
    Restore { filename: String },
    /// Delete snapshots older than retention policy
    Prune {
        #[arg(long, default_value = "2")]
        min_keep: usize,
    },
}

#[derive(Subcommand)]
pub enum ConsoleAction {
    /// Broadcast a message to all players
    Chat { message: String },
    /// List connected players
    Players,
    /// Kick a player by name
    Kick { player: String },
    /// Ban a player by name
    Ban { player: String },
    /// Give an item to a player
    Give { player: String, item: String },
    /// Trigger an in-game world save
    Save,
}
