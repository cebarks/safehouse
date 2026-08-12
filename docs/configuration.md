# Configuration

Safehouse stores all its data under a single root directory, with configuration in a TOML file.

## Directory Layout

```
~/.local/share/safehouse/          # SAFEHOUSE_DIR (default)
├── safehouse.toml                 # Main configuration
├── safehouse.db                   # SQLite database (mod cache, backup records, users, player sessions)
├── backups/                       # World save snapshots (.tar.gz)
├── run/
│   └── server.pid                 # PID file (flock-guarded)
└── logs/                          # Safehouse operational logs
```

PZ server data lives separately:

```
~/pzserver/                        # server_install_dir (configurable)
├── ProjectZomboid64               # PZ server binary
└── ...

~/Zomboid/                         # zomboid_data_dir (default: $HOME/Zomboid)
├── server/
│   ├── servertest.ini             # Server config
│   ├── servertest_SandboxVars.lua # Sandbox settings
│   └── servertest_*.txt           # Server logs
└── Saves/
    └── Multiplayer/
        └── servertest/            # World save data
```

## Data Directory Resolution

Safehouse resolves its root directory in this order:

1. `--data-dir` CLI flag (highest priority)
2. `SAFEHOUSE_DIR` environment variable
3. `~/.local/share/safehouse` (default)

## Configuration Reference

The `safehouse.toml` file is created by `safehouse setup`. All fields have sensible defaults.

```toml
# Path to the PZ server install directory (contains ProjectZomboid64)
server_install_dir = "/home/user/pzserver"

# Server instance name — matches the prefix of files in ~/Zomboid/server/
# e.g. "servertest" reads servertest.ini, servertest_SandboxVars.lua, etc.
server_name = "servertest"

# Path to Zomboid data directory (default: $HOME/Zomboid)
# zomboid_data_dir = "/home/user/Zomboid"

# RCON password — must match RCONPassword in server.ini
rcon_password = "changeme"

# RCON port — must match RCONPort in server.ini (default: 27015)
rcon_port = 27015

# Web UI bind address (default: 0.0.0.0)
web_bind = "0.0.0.0"

# Web UI port (default: 9292)
web_port = 9292

# Discord webhook URL for event notifications (optional)
# discord_webhook_url = "https://discord.com/api/webhooks/..."

# Days to keep backup snapshots before pruning (default: 7)
backup_retention_days = 7

# Send RCON "save" command before creating a backup snapshot (default: true)
backup_rcon_save = true

# Session secret for web UI cookie signing (auto-generated on first `serve`)
# session_secret = "auto-generated-hex-string"
```

### Field Reference

| Field | Type | Default | Description |
| ------- | ------ | --------- | ------------- |
| `server_install_dir` | path | `/opt/pzserver` | Directory containing `ProjectZomboid64` |
| `server_name` | string | `servertest` | PZ server instance name (prefix for config/save files) |
| `zomboid_data_dir` | path? | `$HOME/Zomboid` | Override for the Zomboid data directory |
| `rcon_password` | string | `""` | RCON password (must match server.ini `RCONPassword`) |
| `rcon_port` | u16 | `27015` | RCON TCP port (must match server.ini `RCONPort`) |
| `web_bind` | string | `0.0.0.0` | Web UI listen address |
| `web_port` | u16 | `9292` | Web UI listen port |
| `discord_webhook_url` | string? | none | Discord webhook for notifications |
| `backup_retention_days` | u32 | `7` | Days before `backup prune` deletes old snapshots |
| `backup_rcon_save` | bool | `true` | Trigger world save before creating a backup |
| `session_secret` | string | auto | 64-byte hex secret for cookie signing |

### RCON Password Setup

The RCON password must match in two places:

1. `safehouse.toml` → `rcon_password`
2. `~/Zomboid/server/<name>.ini` → `RCONPassword=`

Set both to the same value. If they don't match, `safehouse console`, `safehouse server stop` (graceful), and the web console won't work.

## Environment Variables

| Variable | Description |
|----------|-------------|
| `SAFEHOUSE_DIR` | Override the safehouse data directory |
| `RUST_LOG` | Override tracing filter (e.g., `safehouse=debug`) |

## Multiple Server Instances

To manage multiple PZ servers, use separate data directories:

```bash
# Server 1
safehouse --data-dir ~/safehouse-pvp server start

# Server 2
safehouse --data-dir ~/safehouse-coop server start
```

Each data directory has its own `safehouse.toml`, `safehouse.db`, and PID file.
