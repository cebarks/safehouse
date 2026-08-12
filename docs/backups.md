# Backups

Safehouse creates `.tar.gz` snapshots of the PZ world save directory.

## How It Works

A backup captures the contents of `~/Zomboid/Saves/Multiplayer/<server_name>/` as a compressed tarball. Files are archived into a temp file first, then atomically renamed to the final name — so a crashed backup never leaves a partial archive.

### Naming Convention

```
servertest_20250728_143022.tar.gz
servertest_20250728_143022_before-mods.tar.gz
```

Format: `<server_name>_<YYYYMMDD>_<HHMMSS>[_<label>].tar.gz`

## Creating Backups

```bash
# Basic backup
safehouse backup create

# With a label
safehouse backup create --label "before-mods"
```

If `backup_rcon_save` is enabled in `safehouse.toml` (default: `true`), safehouse sends an RCON `save` command to the running server and waits 3 seconds before archiving. This ensures the latest world state is flushed to disk.

Backups are stored in `~/.local/share/safehouse/backups/` and recorded in the database with filename, size, and creation timestamp.

## Listing Backups

```bash
safehouse backup list
```

Shows all snapshots sorted newest first with file sizes:

```
servertest_20250728_143022_before-mods.tar.gz      45.2 MB
servertest_20250727_120000.tar.gz                   44.8 MB
```

## Restoring Backups

```bash
safehouse backup restore servertest_20250728_143022.tar.gz
```

This:

1. Stops the server if it's running (graceful shutdown)
2. Extracts the archive into the world save directory
3. Prints a message — start the server manually when ready

**Warning:** Restoring overwrites the current world save. Create a backup of the current state first.

## Pruning Old Backups

```bash
safehouse backup prune
safehouse backup prune --min-keep 5
```

Deletes snapshots older than `backup_retention_days` (default: 7 days), but always keeps at least `--min-keep` (default: 2) snapshots regardless of age.

Pruned files are removed from both disk and the database.

## Automated Backups

Safehouse doesn't include a built-in scheduler, but you can use cron:

```bash
# Every 6 hours
0 */6 * * * /usr/local/bin/safehouse backup create --label "auto"

# Daily prune
0 4 * * * /usr/local/bin/safehouse backup prune
```

## Backup Storage

Backups are stored at `<data_dir>/backups/`. To change this, use `--data-dir` or `SAFEHOUSE_DIR` to point to a directory on a larger disk.

For off-site backups, sync the backups directory to remote storage:

```bash
rsync -av ~/.local/share/safehouse/backups/ backup-server:/backups/pz/
```
