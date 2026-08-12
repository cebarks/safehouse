# Discord Integration

Safehouse sends event notifications to a Discord channel via webhooks.

## Setup

1. In your Discord server, go to **Server Settings → Integrations → Webhooks**
2. Click **New Webhook**, choose a channel, and copy the URL
3. Configure safehouse:

```bash
safehouse webhook --url "https://discord.com/api/webhooks/123456/abcdef..."
```

1. Send a test notification to verify:

```bash
safehouse webhook --test
```

You should see a green "🟢 Server started" embed appear in your Discord channel.

## Events

| Event | Icon | Color | Trigger |
| ------- | ------ | ------- | --------- |
| Server started | 🟢 | Green | `safehouse server start` completes |
| Server stopped | 🔴 | Red | `safehouse server stop` completes |
| Player joined | 👤 | Blue | PZ log: `user '<name>' connected` |
| Player left | 👋 | Gray | PZ log: `user '<name>' disconnected` |
| Backup complete | 💾 | Orange | `safehouse backup create` succeeds |
| Update available | ⬆️ | Purple | (Reserved for future use) |

## How It Works

Notifications are sent as Discord embeds via the webhook URL:

```json
{
  "username": "Safehouse | servertest",
  "embeds": [{
    "title": "👤 Alice joined",
    "color": 39372
  }]
}
```

### Player Event Detection

When `safehouse serve` is running, a background task tails the PZ server log every 2 seconds and parses lines matching:

```
user '<name>' connected
user '<name>' disconnected
```

These events trigger Discord notifications automatically. The watcher handles log file rotation — when PZ creates a new log file after a restart, it resets to the current end of the new file to avoid replaying old events.

### CLI Event Notifications

Server start/stop and backup events are sent directly by the CLI commands. These work regardless of whether `safehouse serve` is running.

## Configuration

The webhook URL is stored in `safehouse.toml`:

```toml
discord_webhook_url = "https://discord.com/api/webhooks/123456/abcdef..."
```

Set it via CLI or edit the file directly. If the field is absent or empty, all notification calls are no-ops.

## Disabling Notifications

Remove or comment out the `discord_webhook_url` line in `safehouse.toml`:

```toml
# discord_webhook_url = "https://discord.com/api/webhooks/..."
```

No restart needed — the next event simply won't send.
