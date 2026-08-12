# CLI Reference

All commands support these global options:

```
--data-dir <PATH>    Override safehouse data directory
--config <PATH>      Override config file path
-v, --verbose        Increase log verbosity (-v = debug, -vv = trace)
```

## setup

Initialize safehouse and install the PZ dedicated server via SteamCMD.

```bash
safehouse setup
safehouse setup --install-dir ~/pzserver
safehouse setup --install-dir ~/pzserver --admin-password mysecret
```

| Option | Default | Description |
|--------|---------|-------------|
| `--install-dir` | `~/pzserver` | PZ server installation directory |
| `--admin-password` | `changeme` | Initial admin password |

Creates `safehouse.toml` in the data directory with default settings. Downloads the PZ dedicated server (Steam app 380870) via SteamCMD using anonymous login.

## server

Manage the PZ server process.

### server start

```bash
safehouse server start
safehouse server start --timeout 120
```

Spawns `ProjectZomboid64`, writes the PID file (with flock), and waits for RCON to become available. The `--timeout` flag (default: 60s) controls how long to wait before giving up.

### server stop

```bash
safehouse server stop
```

Graceful shutdown: sends RCON `save` → waits 3s → sends RCON `quit`. If RCON is unavailable, falls back to SIGTERM. Force-kills with SIGKILL after 40 seconds.

### server restart

```bash
safehouse server restart
```

Equivalent to `stop` followed by `start`.

### server status

```bash
safehouse server status
```

Shows: running/stopped state, PID, server name, install path, and connected players (via RCON).

### server logs

```bash
safehouse server logs
safehouse server logs --lines 50
safehouse server logs --follow
```

| Option | Default | Description |
|--------|---------|-------------|
| `--lines` | `100` | Number of lines to show |
| `--follow`, `-f` | off | Tail the log continuously (Ctrl+C to stop) |

Reads the most recent PZ log file (`~/Zomboid/server/<name>_*.txt`).

## config

Edit PZ server configuration files. Safehouse preserves comments and blank lines when modifying files.

### config show

```bash
safehouse config show
```

Prints the full contents of `server.ini`.

### config set

```bash
safehouse config set MaxPlayers 32
safehouse config set PVP true
safehouse config set ServerWelcomeMessage "Welcome to the server!"
```

Sets a key in `server.ini`. Shows the old and new value.

### config sandbox show

```bash
safehouse config sandbox show
```

Prints the full contents of `SandboxVars.lua`.

### config sandbox set

```bash
safehouse config sandbox set ZombieCount 5
safehouse config sandbox set Zombies.Speed 3
safehouse config sandbox set Zombies.Strength 2
```

Sets a value in `SandboxVars.lua`. Supports dotted keys for nested tables (e.g., `Zombies.Speed`).

### config preset list

```bash
safehouse config preset list
```

Lists saved configuration presets.

### config preset save

```bash
safehouse config preset save vanilla
```

Saves the current mod list from `server.ini` as a named preset.

### config preset apply

```bash
safehouse config preset apply vanilla
```

Applies a saved preset to `server.ini`. Requires a server restart to take effect.

## mods

Manage Steam Workshop mods. PZ requires two identifiers per mod: the Workshop ID (numeric) and the mod's internal folder name.

### mods list

```bash
safehouse mods list
```

Lists all mods from `server.ini` with Workshop IDs, folder names, and cached titles.

### mods add

```bash
safehouse mods add 2392987220 BritasWeaponPack
safehouse mods add 2169435993 Arsenal26
```

Adds a mod to both `WorkshopItems=` and `Mods=` lines in `server.ini`. Fetches and caches metadata from the Steam Workshop API. Requires a server restart to load.

### mods remove

```bash
safehouse mods remove 2392987220
```

Removes a mod from both lists in `server.ini` by Workshop ID.

### mods info

```bash
safehouse mods info 2392987220
```

Fetches and displays Workshop metadata (title, author, description preview) from the Steam API.

### mods profile list

```bash
safehouse mods profile list
```

Lists saved mod collection profiles.

### mods profile save

```bash
safehouse mods profile save "heavy-mods"
```

Saves the current mod list from `server.ini` as a named profile.

### mods profile load

```bash
safehouse mods profile load "heavy-mods"
```

Replaces the mod list in `server.ini` with the saved profile. Restart the server to apply.

## backup

Manage world save backups. Snapshots are `.tar.gz` archives of the world save directory.

### backup create

```bash
safehouse backup create
safehouse backup create --label "before-mods"
```

Creates a timestamped `.tar.gz` snapshot. If `backup_rcon_save` is enabled (default), sends an RCON `save` command before archiving. Uses atomic rename (temp file → final name) to prevent partial archives.

### backup list

```bash
safehouse backup list
```

Lists available snapshots sorted newest first, with file sizes.

### backup restore

```bash
safehouse backup restore servertest_20250728_143022.tar.gz
```

Extracts a snapshot into the world save directory. Automatically stops the server first if it's running.

### backup prune

```bash
safehouse backup prune
safehouse backup prune --min-keep 5
```

Deletes snapshots older than `backup_retention_days` (from config), keeping at least `--min-keep` (default: 2) snapshots regardless of age.

## console

Send RCON admin commands to a running PZ server.

### console players

```bash
safehouse console players
```

Lists currently connected players.

### console chat

```bash
safehouse console chat "Server restarting in 5 minutes!"
```

Broadcasts a message to all connected players.

### console kick

```bash
safehouse console kick "PlayerName"
```

### console ban

```bash
safehouse console ban "PlayerName"
```

### console give

```bash
safehouse console give "PlayerName" "Base.Axe"
```

Gives an item to a player using PZ's internal item name.

### console save

```bash
safehouse console save
```

Triggers an in-game world save.

## webhook

Configure Discord webhook notifications.

```bash
# Set the webhook URL
safehouse webhook --url "https://discord.com/api/webhooks/123/abc"

# Set URL and send a test notification
safehouse webhook --url "https://discord.com/api/webhooks/123/abc" --test

# Test the currently configured webhook
safehouse webhook --test
```

## serve

Start the embedded web management UI.

```bash
safehouse serve
safehouse serve --bind 127.0.0.1 --port 8080
```

| Option | Default | Description |
|--------|---------|-------------|
| `--bind` | from config (`0.0.0.0`) | Listen address |
| `--port` | from config (`9292`) | Listen port |

On first run, prompts for an admin password (hashed with Argon2id). The web UI includes a background log watcher that sends Discord notifications for player events. Handles SIGINT/SIGTERM for graceful shutdown.
