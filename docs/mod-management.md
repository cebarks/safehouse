# Mod Management

Project Zomboid uses two synchronized lists in `server.ini` for Workshop mods:

```ini
WorkshopItems=2392987220;2169435993;2778576730
Mods=BritasWeaponPack;Arsenal26;tsarslib
```

- `WorkshopItems` — semicolon-separated Steam Workshop IDs (numeric)
- `Mods` — semicolon-separated internal mod folder names (must be in the same order)

Safehouse manages both lists together so they stay in sync.

## Syncing from a Steam Collection

The easiest way to manage mods is to curate a Steam Workshop collection and sync from it:

```bash
# Sync by collection ID
safehouse mods sync 3264403312

# Sync from a full Steam URL
safehouse mods sync "https://steamcommunity.com/sharedfiles/filedetails/?id=3264403312"

# Or set the collection in safehouse.toml and just run:
safehouse mods sync
```

This:

1. Fetches all workshop items from the collection
2. Adds new mods and removes any not in the collection
3. Scans downloaded mod directories for folder names (`mod.info` files)
4. Updates both `WorkshopItems=` and `Mods=` in `server.ini`
5. Caches mod metadata (titles, authors) in the database

Mods not yet downloaded are added to `WorkshopItems=` so SteamCMD downloads them on the next server start. Run `safehouse mods sync` again afterward to pick up their folder names.

### Persistent Collection

Set `steam_collection_id` in `safehouse.toml` so you don't need to pass the ID every time:

```toml
steam_collection_id = "3264403312"
```

Then just run `safehouse mods sync` — or click **Sync Collection** in the web UI.

## Finding Mod Information

Every Workshop mod has two identifiers you need:

1. **Workshop ID** — the number in the Steam Workshop URL (e.g., `2392987220`)
2. **Mod folder name** — the internal name declared in the mod's `mod.info` file

To find the folder name:

```bash
# Look it up on the Workshop page, or use safehouse:
safehouse mods info 2392987220
```

The Steam API returns the title and description, but not the internal folder name. Check the mod's Workshop page description or README for the correct `Mods=` name.

## Adding Mods

```bash
safehouse mods add 2392987220 BritasWeaponPack
```

This:

1. Adds `2392987220` to `WorkshopItems=`
2. Adds `BritasWeaponPack` to `Mods=`
3. Fetches metadata from Steam and caches it in the database
4. Requires a server restart to load

Adding a mod that's already present is a no-op (idempotent).

## Removing Mods

```bash
safehouse mods remove 2392987220
```

Removes the Workshop ID from `WorkshopItems=` and the corresponding folder name from `Mods=`. Restart required.

## Listing Mods

```bash
safehouse mods list
```

Shows all installed mods with their Workshop IDs, folder names, and cached titles:

```
Workshop ID          Mod Folder
----------------------------------------
2392987220           Brita's Weapon Pack (BritasWeaponPack)
2169435993           Arsenal(26) Gunstore (Arsenal26)
```

## Mod Profiles

Profiles let you save and switch between different mod configurations quickly.

### Save Current Configuration

```bash
safehouse mods profile save "vanilla-plus"
```

Saves both the `WorkshopItems` and `Mods` lists as a named profile in the database.

### List Profiles

```bash
safehouse mods profile list
```

### Load a Profile

```bash
safehouse mods profile load "vanilla-plus"
```

Replaces the current mod lists in `server.ini` with the saved profile. Restart the server to apply.

## Metadata Cache

Safehouse caches Workshop mod metadata (title, author, description) in `safehouse.db`. This cache is populated when you:

- Run `safehouse mods add` (automatic)
- Run `safehouse mods info` (manual lookup)

The cache prevents repeated Steam API calls for `mods list`.

## Bulk Operations

To add multiple mods at once, chain commands:

```bash
safehouse mods add 2392987220 BritasWeaponPack
safehouse mods add 2169435993 Arsenal26
safehouse mods add 2778576730 tsarslib
safehouse server restart
```

Or save a profile from a working server and load it on another:

```bash
# On server A
safehouse mods profile save "my-modpack"

# On server B (after copying safehouse.db)
safehouse mods profile load "my-modpack"
```
