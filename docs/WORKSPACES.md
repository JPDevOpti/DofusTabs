# DofusTabs: isolated game workspaces

How DofusTabs runs each Dofus client in its own Hyprland workspace, shows the
accounts in the Omarchy bar and switches between them with F1 / F2 / F3.

This replaces the sidebar window. The sidebar design is retired.

## Why the sidebar was dropped

The sidebar needed three pieces of global compositor state: a monitor rule to
reserve a column, the `group:groupbar:enabled` option, and a floating window
kept at a fixed position. All three are runtime overrides, so any Hyprland
config reload wiped them. On this machine `hyprmoncfgd` reloads the config
whenever a monitor rule changes, which the sidebar itself triggered.

The new design owns no global state. It only moves its own game windows.

## Goals

| Goal | How it is met |
| --- | --- |
| Each client isolated from the normal desktop | Game workspaces use ids 11 to 19. Desktop workspaces stay 1 to 10. |
| Accounts visible in the top bar | Bar widget `local.dofus-workspaces`, one class icon per running client. |
| F1 / F2 switch between clients | Keybinds registered only while at least one client is running. |
| F3 returns to the desktop | Focuses the last desktop workspace (1 to 10) that was active. |
| Survives config reloads | Logic lives in the Hyprland config, which is re-evaluated on reload. |

## Tools and why

| Tool | Role | Why this tool |
| --- | --- | --- |
| Hyprland Lua module | Places windows, owns the keybinds, writes the state file | Runs inside the compositor: no daemon, no race with reloads, no process spawn per key press. |
| Omarchy shell plugin (QML) | Renders the icons in the bar | The bar is `omarchy-shell` (Quickshell). User plugins live in `~/.config/omarchy/plugins/`. |
| Rust CLI `dofustabs` | Manages accounts, installs and diagnoses the integration | Keeps the tested domain logic (classes, settings) and the GPU-aware launcher. No window, no webview. |

Tauri, the webview frontend, the tray icon and the sidebar layout code are
removed.

## Verified facts this design relies on

All of these were checked on the target machine (Hyprland 0.56.2).

1. Every client window has class `Dofus.x64` and title `Dofus`. The title does
   not carry the character name or class, so identity cannot come from it.
2. The account id is in the process command line:
   `-logFile .../gamesLogs/dofus-<release>/dofus.<accountId>.log`.
3. Lua inside Hyprland can read `/proc/<pid>/cmdline` and exposes `pid`,
   `class`, `address`, `workspace` and `group` on window objects.
4. `hl.dispatch(hl.dsp.window.move({ workspace = "11", follow = false, window = "address:<addr>" }))`
   moves a client without changing the focused workspace.
5. Available events include `window.open_early`, `window.open`,
   `window.close`, `window.destroy`, `workspace.active` and `config.reloaded`.
6. `hl.bind(key, function() ... end, { description = ... })` accepts a Lua
   function. F1, F2 and F3 are not bound by anything else. F9 is bound
   (dictation) and must never be touched.
7. The existing dots widget only lists workspace ids 1 to 10, so game
   workspaces never appear as desktop dots.
8. `WidgetButton` in the shell renders text only. An icon needs a custom
   `Image` plus `MouseArea`.
9. A monitor rule that omits mode and position moves the monitor. Nothing in
   this design may call `hl.monitor`.
10. Character files: `~/.config/unity3d/Ankama/Dofus/<RELEASE uppercase>/Accounts/<accountId>/Characters/<hash>/dofus.json`,
    where `<hash>` is the lowercase hex MD5 of the character's decimal id
    (verified: `md5("915893715234")` = `92d60605292e2c0cd060f461d152d096` for
    account 190018816). The game rewrites this file after the character
    enters the game; a character not played in the current client session
    keeps an older mtime. Neither the game nor the launcher ever writes the
    character's name or class anywhere on disk.
11. A client's process start time can be read from `/proc/<pid>/stat` field
    22 (`starttime`, clock ticks since boot) plus `/proc/stat`'s `btime`.
12. `hl.timer(fn, { timeout = ms, type = "repeat" })` returns an `HL.Timer`
    that fires reliably; the only way to stop it is `timer:set_enabled(false)`
    (no `remove`/`cancel`/`stop` method exists).

## Architecture

```
accounts.json  --(dofustabs CLI)-->  accounts.lua
                                          |
Dofus client opens --> Hyprland Lua module: read account id from /proc,
                       pick slot, move window to workspace 10 + slot,
                       (un)register F1/F2/F3, write state.json
                                          |
                       state.json  -->  bar widget (icons, focus highlight)
```

### Files

| Path | Owner | Purpose |
| --- | --- | --- |
| `~/.config/dofustabs/accounts.json` | CLI | Source of truth: account id to name, class, preferred slot, and per-character overrides (`characters: { [hash]: { name, class, character_id?, source } }`). |
| `~/.config/dofustabs/accounts.lua` | CLI (generated) | `{ primary_class = "...", accounts = { [id] = { slot, name, class, characters = {...} } } }`. Tolerates the old flat `{ [id] = {...} }` shape too. Never edited by hand. |
| `~/.config/dofustabs/config.json` | CLI | `primary_class` plus mouse settings. |
| `~/.config/hypr/dofustabs.lua` | installer | The Hyprland module. Loaded from `hyprland.lua` with a guarded `dofile`. |
| `$XDG_RUNTIME_DIR/dofustabs/state.json` | Lua module | Running clients: slot, workspace, account, name, class, icon path, pid, active-character hash. |
| `$XDG_RUNTIME_DIR/dofustabs/identity.json` / `identity.lua` | CLI (`identify`, generated) | Runtime state, not configuration (rewritten roughly every 10s while any client runs, only when it changes): each running account's resolved active-character hash. The Lua twin is what the module `dofile`s (no JSON parser in Lua here). |
| `~/.local/share/dofustabs/classes.json` | installer (generated) | `[{ name, icon }]` for all 19 classes, alphabetical; the bar's class picker's source of truth. |
| `~/.config/omarchy/plugins/local.dofus-workspaces/` | installer | Bar widget (`manifest.json`, `BarWidget.qml`). |
| `~/.local/share/dofustabs/icons/*.png` | installer | Class icons plus a generic fallback. |

### Slot rules

- Slots are 1 to 9 and map to workspaces 11 to 19.
- An account with a preferred slot in `accounts.json` always gets it. If that
  slot is currently occupied by a different account (e.g. an unconfigured
  client opened first, or `accounts.json` changed while clients were
  running), the occupant is moved to another slot first, then the owner is
  placed. Moves use `follow = false`, so the user's focused workspace does
  not change unless the moved window happened to be the focused one.
- Every slot preferred by any account in `accounts.json` is reserved, whether
  or not that account is currently running. A client with no preference of
  its own takes the lowest free slot that is not reserved for someone else;
  only once every non-reserved slot is taken does a reserved-but-free slot
  become eligible. This is what stops an unconfigured client from squatting
  on a main account's pinned slot.
- A slot is freed when its window is destroyed.
- Both a config reload and `dofustabs.refresh()` (called by the CLI after
  `accounts set`/`character set`/`primary set`/`remove`) reconcile every
  running client against its account's preferred slot: any client not on its
  preferred slot is moved there, displacing the occupant per the rule above.
  This replaces the earlier behavior where a reload only adopted whatever
  was already on 11-19 and `refresh()` never moved windows.
- **Primary-class override**: if `dofustabs primary set <class>` is
  configured (e.g. `dofustabs primary set selotrop`), the running client
  whose *active* character has that class gets an effective preferred slot
  of 1, overriding any other account's own preference for slot 1
  specifically (their preference for every other slot is unaffected). If two
  running clients both have the primary class, the lowest account id wins
  slot 1 and the other is treated as having no preference at all. This is a
  small pure function, `effective_preferred_slots` (`dofustabs_core::slot` in
  Rust, ported 1:1 into `dofustabs.lua`'s `effective_preferred_slots`); both
  implementations are exercised against the same fixture cases
  (`crates/core/src/slot.rs`'s tests, and `hyprland/selfcheck.lua`).
- The slot-selection rule itself is one small pure function (`choose_slot` in
  `hyprland/dofustabs.lua`): given the currently occupied slots, each
  account's *effective* preferred slot (own preference plus the primary-class
  override above) and the account being placed, it returns the target slot
  with no side effects, so it can be reasoned about and tested in isolation
  from the window-moving code.

### Keys

| Key | From the desktop | From a game workspace |
| --- | --- | --- |
| F1 | Last game workspace used | Previous occupied game workspace, wrapping |
| F2 | Last game workspace used | Next occupied game workspace, wrapping |
| F3 | Nothing | Last desktop workspace used |

The three keys are bound when the first client appears and unbound when the
last one closes. With no client running they reach applications normally.

### Keeping the desktop isolated

Relative workspace navigation (`e+1`, `e-1`) walks every open workspace,
game workspaces included. The module exports `dofustabs.desktop_step(delta)`,
which cycles only within open workspaces 1 to 10. Personal binds that use
`e+1` / `e-1` (the horizontal scroll binds in `bindings.lua`) call it instead.

## CLI

```
dofustabs accounts list
dofustabs accounts set <account-id> [--name <text>] [--class <class>] [--slot <1-9>] [--clear-name] [--clear-class]
dofustabs accounts remove <account-id>
dofustabs character set --account <id> [--character <hash>] [--name <text>] [--class <class>] [--clear-name] [--clear-class]
dofustabs character list
dofustabs character remove --account <id> --character <hash>
dofustabs character from-title --account <id> --title <title>   # internal: called by the module on window.title
dofustabs primary set <class> | clear | show
dofustabs identify [--dry-run]   # internal: polled every ~10s by the module
dofustabs status          # running clients, from state.json
dofustabs doctor          # checks module, widget, icons, binds, config errors
dofustabs install         # installs module, widget, icons, classes.json; idempotent
dofustabs uninstall       # removes them and restores the config lines
dofustabs launch          # starts the Ankama launcher on the dedicated GPU
dofustabs quit [--keep-launcher] [--dry-run]   # closes game clients (and the launcher) with escalating signals
```

Unknown accounts are listed by `dofustabs status` with their id, ready to be
named with `accounts set`.

## Implementation order

1. Hyprland module and bar widget, installed and verified live.
2. Rust workspace cleanup: remove Tauri and the frontend, add the CLI crate,
   keep `core` and `platform` with their tests.
3. Installer, README and final verification.

## Verification checklist

- `hyprctl configerrors` is empty after install and after `hyprctl reload`.
- Starting a client places it on workspace 11 without changing focus.
- A second and third client land on 12 and 13.
- After `hyprctl reload` every client stays on its workspace and the keys
  still work.
- `hyprctl monitors -j` shows the same position and reserved area before and
  after install.
- Closing the last client unbinds F1, F2 and F3.
- The bar shows one icon per client and highlights the focused one.
- `cargo test --workspace` and `cargo clippy --workspace --all-targets` pass.
- `dofustabs identify` resolves each running client's active character
  (matching the hash the game itself uses); `dofustabs status` shows it.
  `identity.json`/`identity.lua` under `$XDG_RUNTIME_DIR/dofustabs/` are only
  rewritten when the resolution actually changes (mtime stable across
  several ~10s polls otherwise).
  `hyprland/selfcheck.lua` (plain `lua`, no compositor needed) passes, and
  its `effective_preferred_slots`/idempotency/shell-escaping fixture cases
  match `crates/core/src/slot.rs`'s Rust tests.
- Right-clicking a client's icon opens the class picker (19 classes from
  `~/.local/share/dofustabs/classes.json` plus "Clear class"); picking one
  labels the account's active character and the icon updates without a
  reload. `omarchy plugin validate` passes and no QML errors appear in the
  journal for this widget after install.

## Recovery

The working tree as it was before this change is saved in
`~/Projects/DOFUS/backup-20260927-pre-workspaces/` (patch of tracked changes
plus a full snapshot). The Tauri application is also in git history up to
commit `5c56b29`.
