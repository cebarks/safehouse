# CLI Reference

```bash
safehouse [OPTIONS] <COMMAND>
```

| Global Option | Description |
| --------------- | ------------- |
| `--data-dir <PATH>` | Safehouse data directory (default: `~/.local/share/safehouse`) |
| `--config <PATH>` | Config file path override |
| `-v`, `--verbose` | Increase verbosity (`-v` debug, `-vv` trace) |

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

### server start

```bash
safehouse server start
safehouse server start --timeout 180
```

| Option | Default | Description |
|--------|---------|-------------|
| `--timeout` | `60` | Seconds to wait for RCON readiness |

Starts the PZ server inside a podman container (`safehouse-pz`). Before launch:

1. Writes `RCONPassword` and `RCONPort` from `safehouse.toml` into `server.ini`
2. Creates and starts the container with volume mounts and port bindings
3. Passes `-cachedir=/zomboid -servername <name> -adminpassword <pass>` to PZ
4. Polls RCON until the server responds or the timeout expires

If the server container exits unexpectedly during startup, safehouse detects it and reports immediately rather than waiting for the full timeout.

### server stop

```bash
safehouse server stop
```

Graceful shutdown sequence:

1. Sends RCON `save` command (world save)
2. Sends RCON `quit` command (graceful shutdown)
3. Waits for the container to exit
4. Falls back to `podman stop` (SIGTERM → PID 1) if RCON quit doesn't work
5. Removes the stopped container

### server restart

```bash
safehouse server restart
```

Equivalent to `server stop` followed by `server start --timeout 60`.

### server status

```bash
safehouse server status
```

Shows whether the container is running, the server name, install directory, and (if running) the connected player count via RCON.

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

Streams logs from the PZ server container via `podman logs`. Works for both running and recently-stopped containers.

## config

### config show

```bash
safehouse config show
```

Prints the full `server.ini` contents.

### config set

```bash
safehouse config set MaxPlayers 32
safehouse config set PVP false
```

Sets a key in `server.ini`. Comment-preserving — existing comments are not stripped.

### config sandbox show

```bash
safehouse config sandbox show
```

Prints the full `SandboxVars.lua` contents.

### config sandbox set

```bash
safehouse config sandbox set ZombieLore.Speed 3
safehouse config sandbox set Zombies.Distribution "Urban Focused"
```

Sets a key in `SandboxVars.lua`. Supports dotted keys for nested Lua tables.

### config preset list

```bash
safehouse config preset list
```

### config preset save

```bash
safehouse config preset save vanilla-plus
```

### config preset apply

```bash
safehouse config preset apply vanilla-plus
```

Applies a saved preset. Requires the server to be stopped — restart after applying.

## mods

### mods list

```bash
safehouse mods list
```

### mods add

```bash
safehouse mods add 2313387159
safehouse mods add 2313387159 --name "Brita's Weapon Pack"
```

Adds a Workshop mod by ID. Updates both `WorkshopItems` and `Mods` in `server.ini`.

### mods remove

```bash
safehouse mods remove 2313387159
```

### mods info

```bash
safehouse mods info 2313387159
```

Fetches mod metadata from the Steam Workshop API.

### mods profile list

```bash
safehouse mods profile list
```

### mods profile save

```bash
safehouse mods profile save vanilla-plus
```

### mods profile load

```bash
safehouse mods profile load vanilla-plus
```

Loads a saved mod profile, replacing the current mod list. Restart the server after loading.

## backup

### backup create

```bash
safehouse backup create
safehouse backup create --label "before-mods"
```

Creates a `.tar.gz` snapshot of the world save directory. If the server is running and `backup_rcon_save` is enabled, sends an RCON save command first.

### backup list

```bash
safehouse backup list
```

### backup restore

```bash
safehouse backup restore servertest_20250728_143022_before-mods.tar.gz
```

Restores a backup. The server must be stopped first.

### backup prune

```bash
safehouse backup prune
safehouse backup prune --days 3
```

Removes backups older than the configured retention period.

## console

### console players

```bash
safehouse console players
```

### console chat

```bash
safehouse console chat "Server restarting in 5 minutes"
```

### console kick

```bash
safehouse console kick PlayerName
```

### console ban

```bash
safehouse console ban PlayerName
```

### console give

```bash
safehouse console give PlayerName Base.Axe
```

### console save

```bash
safehouse console save
```

## webhook

```bash
safehouse webhook --url "https://discord.com/api/webhooks/..."
safehouse webhook --test
```

| Option | Description |
|--------|-------------|
| `--url <URL>` | Set the Discord webhook URL |
| `--test` | Send a test notification |

## serve

```bash
safehouse serve
safehouse serve --bind 127.0.0.1 --port 8080
```

| Option | Default | Description |
|--------|---------|-------------|
| `--bind` | from config | Override bind address |
| `--port` | from config | Override port |

Starts the web management UI. On first run, prompts for an admin password. Auto-generates a session secret if not already set.

The web server also runs a background log watcher for Discord notifications (player join/leave events).
