# Configuration

## Directory Layout

```
~/.local/share/safehouse/          # Safehouse data directory
├── safehouse.toml                 # Main configuration file
├── safehouse.db                   # SQLite database (mods, users, backups, sessions)
├── backups/                       # World backup snapshots (.tar.gz)
├── logs/                          # Safehouse logs
└── run/                           # Runtime state

~/pzserver/                        # PZ dedicated server install (default)
├── ProjectZomboid64               # PZ server binary
├── ProjectZomboid64.json          # JVM configuration
├── start-server.sh                # PZ launch wrapper (not used by safehouse)
├── java/projectzomboid.jar        # Game JAR
├── jre64/                         # Bundled JRE
└── linux64/                       # Native libraries

~/Zomboid/                         # PZ data directory (default)
├── Server/                        # Server config files (capital S)
│   ├── servertest.ini             # Main server settings (RCON, ports, mods)
│   └── servertest_SandboxVars.lua # World rules (zombies, loot, XP, etc.)
├── Saves/Multiplayer/             # World save data
├── Logs/                          # PZ server logs
└── Workshop/                      # Downloaded workshop mods
```

### Data Directory Resolution

Safehouse data directory is resolved in order: `--data-dir` CLI arg → `SAFEHOUSE_DIR` env var → `~/.local/share/safehouse`.

## Configuration Reference

The `safehouse.toml` file is created by `safehouse setup`. All fields have sensible defaults.

```toml
# Path to the PZ server install directory (contains ProjectZomboid64)
server_install_dir = "/home/user/pzserver"

# Server instance name — matches the prefix of files in ~/Zomboid/Server/
# e.g. "servertest" reads servertest.ini, servertest_SandboxVars.lua, etc.
server_name = "servertest"

# Path to Zomboid data directory (default: $HOME/Zomboid)
# zomboid_data_dir = "/home/user/Zomboid"

# RCON password — written to server.ini RCONPassword before each start
rcon_password = "changeme"

# RCON port — written to server.ini RCONPort before each start (default: 27015)
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
| `server_install_dir` | path | `/opt/pzserver` | PZ server binary location |
| `server_name` | string | `"servertest"` | PZ server instance name |
| `zomboid_data_dir` | path? | `~/Zomboid` | Override PZ data directory |
| `rcon_password` | string | `""` | RCON password (written to `server.ini` automatically) |
| `rcon_port` | u16 | `27015` | RCON port (written to `server.ini` automatically) |
| `web_bind` | string | `"0.0.0.0"` | Web UI bind address |
| `web_port` | u16 | `9292` | Web UI port |
| `discord_webhook_url` | string? | none | Discord webhook for notifications |
| `backup_retention_days` | u32 | `7` | Auto-prune backups older than this |
| `backup_rcon_save` | bool | `true` | Send RCON save before backup |
| `session_secret` | string | auto | Web UI cookie signing secret |

### RCON Password Setup

Set `rcon_password` in `safehouse.toml` — safehouse automatically writes `RCONPassword` and `RCONPort` to `~/Zomboid/Server/<name>.ini` before each server start. No manual ini editing required.

The `-adminpassword` flag is also passed to PZ on first run to set the in-game admin password (uses the same `rcon_password` value).

## Environment Variables

| Variable | Description |
|----------|-------------|
| `SAFEHOUSE_DIR` | Override safehouse data directory |
| `RUST_LOG` | Set log level (e.g., `RUST_LOG=safehouse=debug`) |

## Container Image

The PZ server runs inside a podman container built from the `Containerfile` in the repo root. The container image (`safehouse-pz`) includes:

- **fedora-minimal:44** base (~140 MB)
- Runtime libraries for PZ native code (libstdc++, X11, glibc 32-bit)
- SteamCMD for downloading/updating PZ

Build the image:

```bash
podman build -t safehouse-pz -f Containerfile .
```

The PZ server files and world data are volume-mounted, not baked into the image:

| Container path | Host path | Purpose |
| --------------- | ----------- | --------- |
| `/server` | `server_install_dir` | PZ server binaries |
| `/zomboid` | `~/Zomboid` | Server config, saves, logs |

## Multiple Server Instances

Run multiple servers by using separate data directories and server names:

```bash
SAFEHOUSE_DIR=~/.local/share/safehouse-pvp safehouse setup --install-dir ~/pzserver
# Edit the new config: set server_name = "pvp", different ports, etc.
SAFEHOUSE_DIR=~/.local/share/safehouse-pvp safehouse server start
```

Note: each instance needs a unique container name. Currently safehouse uses a fixed container name (`safehouse-pz`), so only one instance can run at a time.
