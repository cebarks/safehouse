# Web UI

Safehouse includes an embedded web dashboard built with actix-web and HTMX. All assets (CSS, JavaScript) are compiled into the binary — no external files needed at runtime.

## Starting the Web UI

```bash
safehouse serve
```

On first run, you'll be prompted to set an admin password:

```
Set admin password: ********
Admin user created.
Safehouse starting on http://0.0.0.0:9292
```

Navigate to `http://your-server:9292` and log in with username `admin` and the password you set.

## Pages

### Dashboard (`/`)

Shows at a glance:

- **Server status** — running (green) or stopped (red) indicator
- **Server name** — from `safehouse.toml`
- **Connected players** — live count via RCON (when server is running)
- **Recent log lines** — last 20 lines from the PZ server log

### Config (`/config`)

Displays the full contents of `server.ini` and `SandboxVars.lua`. Provides forms to set individual keys:

- **server.ini** — flat `Key=Value` pairs (e.g., `MaxPlayers`, `PVP`)
- **SandboxVars.lua** — supports dotted keys for nested tables (e.g., `Zombies.Speed`)

Changes are written to disk immediately. Restart the server to apply most settings.

### Mods (`/mods`)

- **Add mod** — enter a Workshop ID and mod folder name
- **Mod list** — shows all installed mods with Workshop ID, folder name, and cached title
- **Remove** — removes a mod from `server.ini`
- **Profiles** — lists saved mod collection profiles

Metadata is fetched from the Steam Workshop API and cached in the database.

### Backups (`/backups`)

- **Create backup** — creates a `.tar.gz` snapshot with an optional label
- **Snapshot list** — shows all backups with filename and size

### Console (`/console`)

An interactive RCON console. Type a command, click Send, and see the response inline (powered by HTMX — no page reload).

Common commands: `players`, `servermsg "text"`, `kickuser "name"`, `save`.

### Logs (`/logs`)

Displays the last 200 lines from the most recent PZ server log file.

## Authentication

- Sessions use signed cookies (actix-session with CookieSessionStore)
- The session secret is a 64-byte hex string, auto-generated on first `serve` and stored in `safehouse.toml`
- Session TTL is 7 days
- Passwords are hashed with Argon2id
- All pages except `/login` require authentication — unauthenticated requests redirect to `/login`

## Background Log Watcher

When `serve` is running, a background task monitors the PZ server log:

- Checks for new log lines every 2 seconds
- Parses player connect/disconnect events
- Sends Discord notifications (if a webhook URL is configured)
- Handles log file rotation (resets position when PZ creates a new log file)

## Customization

The web UI uses a dark theme with these CSS classes available for customization:

| Class | Element |
| ------- | --------- |
| `.nav` | Top navigation bar |
| `.status-card.running` | Green-bordered status card |
| `.status-card.stopped` | Red-bordered status card |
| `.btn-primary` | Primary action buttons (red accent) |
| `.btn-danger` | Destructive action buttons |

To customize styles, modify `src/assets/style.css` and rebuild. The CSS is embedded in the binary at compile time via `rust-embed`.
