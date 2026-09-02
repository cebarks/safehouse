# Survivor — Server-Authoritative Mod Distribution (Design)

**Date:** 2026-08-29
**Amended:** 2026-08-29 — added verified B42 `-modfolders` lever: workshop scanning can be disabled outright at launch, demoting unsubscribe to disk reclaim
**Amended:** 2026-08-29 — design review: plain-copy releases, `set-release` version control, dedicated steamcmd image + slimmed PZ image, fetch session-loop spec, endpoint hardening, installer repair mode
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
│  surv fetch: throwaway podman containers (dedicated          │
│  safehouse-steamcmd image) → persistent staging bind mount   │
│        │                                                     │
│        ▼                                                     │
│  surv cut: plain-copy snapshot + xxh3 manifest + changelog   │
│        ▼                                                     │
│  releases/<id>/files/   ← IMMUTABLE                         │
│  active → releases/<id> ← set by `server set-release`        │
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
- **`safehouse surv fetch`:** host-side driver loop in safehouse spawns
  sequential throwaway containers from the dedicated `safehouse-steamcmd`
  image (see "Container fetch mode & images" below). Each container runs
  one steamcmd session: `+login anonymous`, then
  `+workshop_download_item 108600 <id> validate` for a batch of ~25 IDs
  from the DB. Never one login per item (Steam rate-limits rapid logins,
  code 84); never parallel sessions. Downloads into the persistent staging
  bind mount `surv/staging/`; `validate` against existing staging is the
  resume mechanism and makes repeat fetches incremental. The live server's
  workshop content is never touched.
  - **Result parsing:** steamcmd injects ANSI escape sequences into its
    output (known landmine — arkmanager's parser broke on this); strip
    ANSI, then attribute success per item via `Success. Downloaded item
    <id>`. Any batch ID without a success line failed; a crashed or
    segfaulted session fails all its unconfirmed IDs.
  - **Retries:** failed IDs requeue into the next session; default 5
    attempts per item. steamcmd workshop downloads run at ~100 Mbps with a
    ~5-minute timeout window, so the 5.48 GiB mod legitimately needs
    multiple validate-resume attempts — multi-attempt completion is the
    normal path for large items, not an error. `surv fetch` fails loudly
    at the end listing any IDs still failing.
  - **Timing:** first full fetch (~21 GiB) ≈ 30–60 min; incremental
    fetches minutes.
- **`safehouse surv cut [--note …]`:** for each workshop ID in the DB, flatten
  `staging/…/content/108600/<wsid>/mods/<ModID>/` → `releases/<id>/files/<ModID>/`
  via **plain copy** (releases fully own their files — steamcmd rewriting
  staging can never touch an existing release; storage is cheap at this
  scale). Symlinks and special files in the source tree **fail the cut**,
  naming the offending mod + wsid (they would break server-side loading,
  client-side writes, and endpoint containment anyway). Parallel xxh3-64
  hashing of the copied tree — the manifest hashes the *release copy*,
  then a re-hash verification pass runs before the cut completes; a
  mismatch fails the cut loudly. Records: release id (date stamp + serial,
  e.g. `2026.08.30-r1`), PZ game build, per-file `{path, size, xxh3}`,
  per-mod metadata (DB titles/authors), `load_order` from the managed
  `Mods=` line (B42 backslash-prefixed).
- **Changelog:** diff against previous release using staging metadata +
  workshop update timestamps: added / removed / updated mods. Displayed in
  client update GUI, web UI releases page, and Discord webhook (existing
  safehouse infra).
- **`active` semantics:** clients sync to the release the server is *running*,
  not the latest cut. `active` is flipped **only after the server has
  booted healthily on that release** (RCON responds) — there is no state
  where `active` names a release nothing is serving. Version changes
  happen solely through `safehouse server set-release <id>` (see
  Self-pinning); plain `server start`/`restart` always boot the current
  `active`. Adoption ceremony: fetch → cut → review changelog →
  `server set-release <id>` → Discord announcement → players converge on
  next launch. Rollback = `server set-release <previous-id>`.

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
│   │   │   └── files/<ModID>/…   (plain copies)
│   │   └── …
│   └── active -> releases/2026.08.30-r1
```

`staging/` is a host bind mount (mounted `:Z` like the existing
`container.rs` binds) persisting the **entire** steamcmd tree —
`steamapps/workshop/content/108600/<wsid>/` *and*
`steamapps/workshop/appworkshop_108600.acf` plus steamcmd's own state.
The ACF is what `validate` diffs against; losing it turns the next fetch
into a full 21 GiB re-download. safehouse never moves or deletes staging
content — `surv cut` copies out, nothing else touches it.

### Endpoints (existing actix app; PSK via `X-Surv-Key` header; TLS via nginx)

| Endpoint | Purpose |
| --- | --- |
| `GET /surv/active` | release id + minimal metadata; the client's only cost on an up-to-date launch |
| `GET /surv/manifest?release=` | manifest.json, gzip, ETag = release id |
| `GET /surv/file/<path>` | whole file; **Range support for resume** (multi-GB mods), not deltas; hardened via manifest allowlist (below) |
| `GET /surv/client/<os>` | installer + agent jar distribution |

PSK configured in `safehouse.toml` under `[surv]`. PSK is obfuscation-grade
auth (baked into client builds) — bandwidth gate against strangers, nothing
more. It is extractable by design ⇒ `/surv/file` is effectively
unauthenticated file serving and is hardened accordingly:

1. **Manifest allowlist:** the requested path must be an exact key in the
   active release's manifest, else 404. The served path is only ever built
   from manifest-validated keys — traversal is structurally impossible,
   not filter-dependent.
2. **Containment:** join the validated key to the release `files/` dir,
   canonicalize, assert the result stays inside (defense against manifest
   bugs / TOCTOU).
3. **Cut-time hygiene:** symlink/special-file rejection in `surv cut`
   guarantees the served tree is plain files only.

PSK comparison is constant-time (`subtle`/ct_eq).

### Self-pinning

`safehouse server set-release <id>` — the **sole** version-change mechanism
("make `<id>` live", convergent):

1. Validates the release exists + manifest re-hash passes.
2. If the server is running on a different release: graceful stop (existing
   RCON `save` + `quit` path). Already running on `<id>` → no-op.
3. Mounts `releases/<id>/files/` as the container's mod source (direct
   bind mount, not symlink).
4. Writes `server.ini`: `WorkshopItems=` **empty**, `Mods=` from manifest
   `load_order`; adds `-modfolders mods` to server launch args so the
   historical `steamapps/workshop/content` tree on the server can never
   shadow release files.
5. Boots, waits for RCON readiness (existing health check).
6. **Only on healthy boot** flips `active` → `releases/<id>`.

Plain `server start` / `restart` always boot the current `active` — no
release argument; with no `active` yet, they error with a hint to run
`set-release`. `/surv/active` serves the last-active release regardless of
whether the server is currently running (clients are fail-open; the join
check enforces). Steam touches nothing at server runtime. **Pilot-verify
item #1:** B42 dedicated boots and serves with empty `WorkshopItems=` +
local mods.

### Container fetch mode & images

**`safehouse-steamcmd` (new, dedicated image):** fedora-minimal base, i686
runtime libs, steamcmd install + build-time self-update warm-up (the
existing recipe extracted from the current Containerfile). Entrypoint
wrapper runs `steamcmd.sh +quit` first to absorb any pending steamcmd
self-update — without this, a stale image can hit mid-session self-updates
that restart steamcmd and drop its arguments ("Command aborted") — then
`exec`s the real session. safehouse spawns one throwaway container per
fetch session; mounts = staging bind mount only; no ports.

**`safehouse-pz` (slimmed):** drops the i686 packages, the steamcmd
install block, and `/steamcmd` from PATH; keeps the 64-bit PZ runtime
libs, volumes, ports, and the Java-as-PID-1 entrypoint. Purely "run the
server". `run_steamcmd_install()` (PZ install/update during setup)
re-points at `safehouse-steamcmd` with the same `/server:Z` bind — its own
commit, with a setup regression check.

Fails loudly on partial downloads (per the `surv fetch` spec above);
resumable via re-run (`validate`).

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

**Installer-as-repair (Windows failure recovery).** Broken zbNative ⇒
plain `-javaagent:` fails ⇒ **the game does not launch on Windows at all**
(JVM aborts during agent load; no window, no Survivor code ever runs —
nothing in-JVM can detect or fix it). Players are non-technical; recovery
must not require a terminal. Running the installer on an already-installed
machine defaults to repair: re-detect ZB (tolerating unusual paths),
re-assert arg order across all surfaces, or — if ZB is gone entirely —
strip the agent args to restore a bootable game (player loses auto-sync
until ZB is fixed). Double-clickable, idempotent; the Discord answer is
"re-run the installer". Idempotency also covers arg-order races: if a ZB
update rewrites `ProjectZomboid64.json` vmArgs, re-running the installer
fixes the chain. (A background watchdog would make this zero-touch but
ships an elevated recurring task from an unsigned binary — malware-shaped
to AV; deferred as the escalation path if pilot shows zbNative breakage is
common.)

## Migration & Rollout

| Phase | Who | What |
| --- | --- | --- |
| 0 | admin | Build; authoritative `safehouse mods sync`; cross-check DB vs `pz-mod-list/reports/mod-names.json` (~446 expectation); cut r1; pilot (§Verification) |
| 1 | group | Installer (Workshop storefront / GH release) → bootstrap sync. 2.5/1.25 Gbit fiber: no staggering needed (~140s/player solo at the 1.25 Gbit upload line rate, shared across concurrent bootstraps) |
| 2 | group | Optional disk reclaim: one-click unsubscribe-all on collection `3786678808` → Steam deletes ~21 GiB/player. With `-modfolders mods` set, subscriptions are inert — this is storage hygiene, not correctness; sync first anyway |
| 3 | admin | `safehouse server set-release r1` → cutover complete |

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

**Pilot topology:** admin Mac client + one Windows volunteer + one Linux
client (we have Linux players) + server.
Scenarios: first bootstrap → delta update (one mod changed) → rollback
(`set-release <previous-id>`) → offline launch (server unreachable → game
boots anyway) → corruption (flip bytes → client re-fetches).
**Fetch pilot (server-side, before group rollout):** time a full 446-item
fetch on mars and record the retry rate; an immediate re-run with zero
upstream changes should download ~nothing (idempotency); kill the
container mid-download of the 5.48 GiB item and confirm validate-resume
completes it across attempts.
**Success bar:** one week, zero version-mismatch join failures.

## Rejected Alternatives

| Alternative | Why rejected |
| --- | --- |
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
- **2026-08-29 review amendments:**
  - Release copy semantics: **plain copy** (storage is cheap); xxh3 manifest
    hashed against the release copy + re-hash verification at cut; reflink
    (`FICLONE`, verified working on mars's XFS) noted as future improvement
  - Version control: **`server set-release <id>`** is the sole
    version-change mechanism; `active` flips only after RCON-healthy boot;
    `start`/`restart` always boot the current `active`; `/surv/active`
    serves last-active regardless of server state
  - Images: dedicated **`safehouse-steamcmd`** image + slimmed
    `safehouse-pz`; both keep the **fedora-minimal:44** base (Project
    Hummingbird rebuild deferred post-pilot)
  - Fetch pipeline: host-side session loop — ~25 items/session, sequential
    sessions, never per-item login (rate-limit code 84), ANSI-stripped
    output parsing, `validate` on every attempt (resume), 5 attempts/item
    default, loud failure listing; the ~100 Mbps / 5-min timeout regime
    makes multi-attempt completion of large items normal
  - Staging: host bind mount `:Z` under the safehouse data dir; persist the
    full steamcmd tree incl. `appworkshop_108600.acf`; never reorganize
  - `/surv/file` hardening: manifest allowlist → canonicalize-and-contain →
    cut-time symlink/special-file rejection; constant-time PSK compare
  - Windows zbNative breakage ⇒ game does not launch; recovery =
    installer-as-repair (no terminal); background watchdog deferred
  - Pilot adds one Linux client

## Risks / Open Questions

- Empty-`WorkshopItems=` local-load on B42 dedicated (pilot #1 — if it fails,
  fallback: keep `WorkshopItems=` populated but mount release files over the
  workshop content paths via symlinks/bind-mounts; server-side
  `-modfolders mods` removes shadowing risk in both variants)
- Losing/corrupting `appworkshop_108600.acf` in staging silently turns the
  next fetch into a full 21 GiB re-download (`validate` diffs against it) —
  back it up with the staging tree
- Case sensitivity: preserve exact author casing; Linux clients are
  case-sensitive (safehouse's existing `fix-case` knowledge applies)
- Players with ZB installed via unusual paths → installer detection must be tolerant

## Future Improvements (deferred)

- **Reflink copies** for `surv cut` instead of plain copy — verified working
  on mars's XFS (`FICLONE`); near-zero disk cost with full release
  independence. Only worth it if release retention starts costing real disk.
- **Project Hummingbird base** for `safehouse-pz` (Red Hat hardened
  distroless images; `core-runtime` + builder-stage library closure).
  Deferred: PZ bundles its own JRE so only the glibc/lib layer would apply,
  distroless hampers debugging of TIS's shifting native binaries, and the
  base must stay frozen through the pilot. `safehouse-steamcmd` stays
  fedora-minimal regardless (steamcmd needs a shell; no distroless win).
- **Background watchdog** (scheduled task) for Windows zbNative drift —
  true zero-touch repair; deferred for AV friction (an elevated recurring
  task from an unsigned binary is malware-shaped). Escalate if the pilot
  shows breakage is common.
