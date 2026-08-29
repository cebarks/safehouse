# Survivor — Server-Authoritative Mod Distribution (Design)

**Date:** 2026-08-29
**Amended:** 2026-08-29 — added verified B42 `-modfolders` lever: workshop scanning can be disabled outright at launch, demoting unsubscribe to disk reclaim
**Status:** Validated design, pre-implementation
**Feature name:** Survivor · CLI namespace: `safehouse surv` · Client artifact: `survivor-agent.jar`

## Problem

Zanctuary (PZ B42 dedicated, collection `3786678808`, ~446 workshop items,
~21 GiB) is plagued by constant mod updates: Steam silently auto-updates
subscribed workshop items on clients at connect time, while the server holds
whatever it loaded at startup. With 7–31 items updating per day, someone is
always version-skewed and cannot join until a full coordinated restart.
Root cause confirmed: PZ clients check the Workshop directly when joining and
pull the latest versions; the server does not re-read mods until restart.

## Goals

1. **Version pinning:** mod set changes only when the admin deliberately cuts a release.
2. **Version tracking:** every release is an immutable, identifiable snapshot with an auto-generated changelog.
3. **Zero-effort clients:** non-technical Windows/Linux players update automatically at game launch, with progress UI. No separate sync step, no restarts.
4. **Consistency by construction:** clients converge on the release the server is *running*; join-skew becomes impossible within a release.

## Non-goals (v1)

- Block-level/byte deltas (whole-file sync via checksum compare instead).
- P2P seeding (admin's 2.5/1.25 Gbit fiber hosts everything).
- In-game consent prompts (silent auto-sync; escape hatches only on failure).
- Per-player attribution / real security (PSK is an anti-abuse gate, extractable from the client by design).

## Architecture Overview

```
                      ADMIN
                        │
   safehouse mods sync  │  (existing: Steam collection → safehouse DB + server.ini)
                        ▼
┌──────────────────────────── mars ────────────────────────────┐
│  surv fetch: throwaway podman container (steamcmd only,      │
│  no PZ binary) → persistent staging volume                   │
│        │                                                     │
│        ▼                                                     │
│  surv cut: hardlink snapshot + xxh3 manifest + changelog     │
│        ▼                                                     │
│  releases/<id>/files/   ← IMMUTABLE                         │
│  active → releases/<id> ← set by `server start --release`    │
│        │                       │                             │
│        ▼                       ▼                             │
│  PZ server loads FROM     actix endpoints (X-Surv-Key PSK):  │
│  release dir; Workshop-   /surv/active /surv/manifest        │
│  Items= EMPTY              /surv/file/<path> /surv/client/* │
└───────────────────────────────┬──────────────────────────────┘
                                │ HTTPS (existing nginx/TLS)
                ┌───────────────┼───────────────┐
                ▼               ▼               ▼
          win client       linux client      mac client
          Survivor agent (premain) → Zomboid/mods/ (local mods)
```

Players launch with `-modfolders mods` (see "B42 `-modfolders` lever" below):
the game never scans workshop content, so subscription state is irrelevant and
Steam auto-update is removed from the equation entirely. Mods live as local
mods in `Zomboid/mods/`. Unsubscribing (phase 2) is disk reclaim, not a
correctness step. Byte-identical files on server + clients satisfy version
checks (including `DoLuaChecksum`) by construction.

### B42 `-modfolders` lever (verified 2026-08-29)

PZ B42 accepts `-modfolders {folders}`, controlling which mod sources are
scanned and their priority. Verified against the actual dedicated build on
mars (`java/projectzomboid.jar` strings): `MainScreenState` parses the flag →
`ZomboidFileSystem.setModFoldersOrder`; the default order string is
**`workshop,steam,mods`** — i.e. workshop copies *shadow* local mods of the
same ID when both exist, so the flag is mandatory for this design, not
optional. With `-modfolders mods`, `steamapps/workshop/content` is never
scanned: subscribed items are fully inert (no auto-update risk, no duplicate
IDs), and unsubscribing becomes optional disk reclaim.

Tradeoff: the flag is process-global — a player using the same PZ install for
other workshop-based servers must drop the flag for those sessions (their
subscriptions remain intact unless they ran phase-2 unsubscribe).

## Release Model

- **Source of truth:** safehouse's SQLite mod registry + managed `server.ini`
  (`WorkshopItems=`/`Mods=`), populated by the existing `safehouse mods sync`
  from collection `3786678808`. The legacy `pz-mod-list/mods.txt`/`order.txt`
  files are **not** part of this pipeline.
- **`safehouse surv fetch`:** throwaway podman container from the existing
  `safehouse-pz` image, entrypoint overridden to a batched steamcmd session
  (`+login anonymous`, `+workshop_download_item 108600 <id> validate` per ID
  from the DB). Downloads into persistent staging volume `surv/staging/`.
  `validate` against existing staging makes repeat fetches incremental.
  The live server's workshop content is never touched.
- **`safehouse surv cut [--note …]`:** for each workshop ID in the DB, flatten
  `staging/…/content/108600/<wsid>/mods/<ModID>/` → `releases/<id>/files/<ModID>/`
  via hardlinks (near-zero disk cost until files change). Parallel xxh3-64
  hashing of the full tree. Records: release id (date stamp + serial, e.g.
  `2026.08.30-r1`), PZ game build, per-file `{path, size, xxh3}`, per-mod
  metadata (DB titles/authors), `load_order` from the managed `Mods=` line
  (B42 backslash-prefixed).
- **Changelog:** diff against previous release using staging metadata +
  workshop update timestamps: added / removed / updated mods. Displayed in
  client update GUI, web UI releases page, and Discord webhook (existing
  safehouse infra).
- **`active` semantics:** clients sync to the release the server is *running*,
  not the latest cut. `safehouse server start --release <id>` flips `active`
  (symlink). Adoption ceremony: fetch → cut → review changelog →
  `server restart --release <id>` → Discord announcement → players converge
  on next launch. Rollback = re-activate previous release + restart.

## Server Side

### Store layout

```
<safehouse data dir>/
├── surv/
│   ├── staging/              # steamcmd workshop mirror (persistent)
│   ├── releases/
│   │   ├── 2026.08.30-r1/
│   │   │   ├── manifest.json
│   │   │   ├── changelog.json
│   │   │   └── files/<ModID>/…   (hardlinks)
│   │   └── …
│   └── active -> releases/2026.08.30-r1
```

### Endpoints (existing actix app; PSK via `X-Surv-Key` header; TLS via nginx)

| Endpoint | Purpose |
|---|---|
| `GET /surv/active` | release id + minimal metadata; the client's only cost on an up-to-date launch |
| `GET /surv/manifest?release=` | manifest.json, gzip, ETag = release id |
| `GET /surv/file/<path>` | whole file; **Range support for resume** (multi-GB mods), not deltas |
| `GET /surv/client/<os>` | installer + agent jar distribution |

PSK configured in `safehouse.toml` under `[surv]`. PSK is obfuscation-grade
auth (baked into client builds) — bandwidth gate against strangers, nothing more.

### Self-pinning

`safehouse server start --release <id>`:
1. Mounts/symlinks `releases/<id>/files/` as the container's mod source.
2. Writes `server.ini`: `WorkshopItems=` **empty**, `Mods=` from manifest `load_order`.
3. Flips `active` symlink.
4. Adds `-modfolders mods` to server launch args so the historical
   `steamapps/workshop/content` tree on the server can never shadow release
   files.

Steam touches nothing at server runtime. **Pilot-verify item #1:** B42
dedicated boots and serves with empty `WorkshopItems=` + local mods.

### Container fetch mode

New bollard command profile on the existing image: entrypoint = steamcmd
download script; mounts = staging volume only. No PZ install/boot, no ports.
Fails loudly on partial downloads; resumable via re-run (`validate`).

## Client Side — Survivor Agent

**Single artifact, dual mode:** `survivor-agent.jar` — `premain()` when the
game launches, `main()` when run standalone (bootstrap/repair via the bundled
`jre64`, so no system Java requirement). Java 17.

### Launch flow

1. `premain` → `GET /surv/active` (~2s timeout, **fail-open**: unreachable →
   log warning, boot anyway; the game's own join check remains enforcement).
2. Compare against local `Zomboid/surv/state.json` `{release, path: {size,
   xxh3, mtime}}`. Match → proceed instantly (no hashing).
3. Drift → spawn **side-car UI process** (bundled jre64 launched *without*
   `-Djava.awt.headless` — the game JVM itself runs headless, so in-process
   Swing is out; ZB's signing dialog proves side-car windows work): release
   name, changelog, progress bar. Premain blocks on it.
4. Download changed/new files (concurrency pool ~32–64, HTTP/2 keep-alive,
   Range-resume), xxh3-verify each whole file, write into `Zomboid/mods/`,
   remove owned files absent from manifest (ownership = tracked in
   `state.json`; never touch untracked files).
5. Update `state.json`, exit side-car, return from `premain` — **the launch
   that triggered the update runs the updated set**. Zero restarts, ever.

Escape hatch (failure paths only, not happy path): "continue without
updating" when the download fails or the server is unreachable mid-sync.

### Agent injection & ZombieBuddy chaining

- Injected via launch args on the same surfaces ZB patches:
  `ProjectZomboid64.json` vmArgs, Steam launch options, `ProjectZomboid64.bat`.
- Installer also inserts `-modfolders mods` on the same surfaces (verified B42
  flag; see lever section). Without it, residual workshop copies shadow synced
  local mods — default scan order is `workshop,steam,mods`.
- **Argument order is the chain:** agents' `premain` run in arg order before
  `main()`. Installer inserts Survivor's agent arg **before** ZB's → sync
  completes → ZB initializes and discovers the freshly updated mod set
  (including ZB java-mod jars). No runtime chaining code.
- **Windows landmine:** plain `-javaagent:` fails in PZ's bundled JRE
  (`jre64\bin\instrument.dll` deps missing from DLL path). ZB's `zbNative.dll`
  (`-agentlib:zbNative`) fixes the DLL path process-wide; Survivor requires
  ZB installed on Windows and inserts its plain `-javaagent:` after it.
  Installer verifies; refuses with instructions otherwise. macOS/Linux:
  plain `-javaagent` works.

### Config & self-update

- `Zomboid/surv/surv.toml`: server URL, PSK (build-baked default + file
  override), `auto_update_max_gb` ceiling (exceed → log + Discord nudge,
  suggest re-running installer), log level.
- Log: `Zomboid/surv/surv.log`.
- Self-update: agent writes `survivor-agent.jar.new`, swaps on next launch
  (Windows file-lock workaround).
- Distribution: Workshop item as **storefront** (description, changelog,
  installer), GitHub releases as mirror. Runtime lives at the stable
  `Zomboid/surv/` path, **never** in `steamapps/workshop/content/` (Steam
  cache semantics — same precedent as ZB's installer copying jars to the
  game dir).

### Installer

Small cross-platform installer (ZB-installer UX: detect → preview → confirm):
1. Verify ZB present (Windows: zbNative).
2. Copy agent jar to `Zomboid/surv/`.
3. Insert Survivor arg before ZB arg and `-modfolders mods` on all patched
   launch surfaces.
4. Run first bootstrap sync (21 GiB) with the side-car GUI.

## Migration & Rollout

| Phase | Who | What |
|---|---|---|
| 0 | admin | Build; authoritative `safehouse mods sync`; cross-check DB vs `pz-mod-list/reports/mod-names.json` (~446 expectation); cut r1; pilot (§Verification) |
| 1 | group | Installer (Workshop storefront / GH release) → bootstrap sync. 2.5/1.25 Gbit fiber: no staggering needed (~70s/player at line rate) |
| 2 | group | Optional disk reclaim: one-click unsubscribe-all on collection `3786678808` → Steam deletes ~21 GiB/player. With `-modfolders mods` set, subscriptions are inert — this is storage hygiene, not correctness; sync first anyway |
| 3 | admin | `safehouse server restart --release r1` → cutover complete |

ZB runtime safety: ZB's jars live in the game dir (its installer copied them),
so unsubscribing its workshop item breaks nothing; the ZB *mod* itself
(lua + media) ships inside the pinned release like any other mod.

Windows specifics: vmArgs paths quoted for `%USERPROFILE%` spaces; unsigned
installer friction mitigated via Workshop/GH distribution + documented
one-liner fallback.

## Verification & Pilot

**Assumptions to prove experimentally before group rollout:**
1. B42 dedicated boots & serves with empty `WorkshopItems=` + local mods *(the linchpin)*
2. Clients join with `-modfolders mods` set (subscription state irrelevant); `DoLuaChecksum` satisfied by byte-identical local files
3. Side-car UI process spawns from premain despite game JVM headless (Win + Linux)
4. Premain blocking + arg-order chain-loading works with ZB (freshly synced ZB jars load correctly)
5. Plain `-javaagent` on Windows loads once zbNative is present

**Pilot topology:** admin Mac client + one Windows volunteer + server.
Scenarios: first bootstrap → delta update (one mod changed) → rollback
(re-activate previous release) → offline launch (server unreachable → game
boots anyway) → corruption (flip bytes → client re-fetches).
**Success bar:** one week, zero version-mismatch join failures.

## Rejected Alternatives

| Alternative | Why rejected |
|---|---|
| One squashed workshop item | 21 GiB vs ~1 GiB workshop item caps; largest single mod is 5.48 GiB; republish = full re-download for everyone |
| Split pinned workshop bundles | Re-uploading third-party mods (ToS/takedown risk); republish churn; 5.48 GiB mod can't fit any bundle |
| Syncthing | Receive-only folders + P2P were attractive, but players still need Steam for the game and the agent model gives better UX/version control |
| Standalone sync binary | Loses to the agent on timing: agent syncs *before* mod load inside the same launch; binary needs separate run + coordination |
| rsync | Non-technical Windows users |
| Workshop as agent runtime path | Steam cache semantics (unsubscribe deletes, updates rewrite); ZB's own installer sets this precedent |
| In-process GUI via headless property flip | Side-car process is robust and doesn't touch game JVM flags |
| Per-mod tar.zst archive fast path | Complexity for little gain; whole-file sync + concurrency pool saturates 2.5 Gbit fine |

## Decisions Log

- Checksum: **xxh3-64** (drift/corruption detection, not adversarial integrity; PSK+TLS guard transport)
- **`-modfolders mods`** on all client + server launch surfaces (B42-verified 2026-08-29 against the mars build): disables workshop scanning outright; the default order `workshop,steam,mods` would otherwise let stale workshop copies shadow pinned local mods
- Client syncs to **active** release (server-running), not latest cut
- Load order source: safehouse-managed `Mods=` line (B42 `\`-prefixed)
- Release ids: `<date>-r<serial>`; retention policy mirrors backup pruning
- B42 detail: `Mods=` entries need backslash prefix; `DoLuaChecksum` compares loaded Lua host-vs-client at join

## Risks / Open Questions

- Empty-`WorkshopItems=` local-load on B42 dedicated (pilot #1 — if it fails,
  fallback: keep `WorkshopItems=` populated but mount release files over the
  workshop content paths via symlinks/bind-mounts; server-side
  `-modfolders mods` removes shadowing risk in both variants)
- Hardlinks across container volume mounts (fallback: reflink/plain copy)
- Case sensitivity: preserve exact author casing; Linux clients are
  case-sensitive (safehouse's existing `fix-case` knowledge applies)
- Players with ZB installed via unusual paths → installer detection must be tolerant
