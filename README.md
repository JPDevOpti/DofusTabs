# DofusTabs

Isolated Dofus game workspaces for Hyprland: every game client gets its own
workspace (11-19), the Omarchy bar shows one icon per running account, and
`F1`/`F2`/`F3` switch between clients and back to the desktop.

See [`docs/WORKSPACES.md`](docs/WORKSPACES.md) for the full design.

## Requirements

- Hyprland with the Lua config format (`~/.config/hypr/hyprland.lua`).
- The Omarchy shell (`omarchy-bar`, `~/.config/omarchy/shell.json`).
- Rust to build the CLI.

## Install

```sh
git clone <this repo>
cd dofustabs
./install.sh
```

This builds the `dofustabs` binary in release mode, installs it to
`~/.local/bin/dofustabs`, and runs `dofustabs install`, which:

- writes `~/.config/hypr/dofustabs.lua` (the Hyprland module),
- adds a guarded loader block to `~/.config/hypr/hyprland.lua` (only if not
  already present),
- installs the bar widget to `~/.config/omarchy/plugins/local.dofus-workspaces/`
  and registers it in the bar layout (`omarchy bar put`, only if not already
  registered),
- installs the class icons to `~/.local/share/dofustabs/icons/`,
- creates `~/.config/dofustabs/accounts.json` and the generated
  `accounts.lua` if they do not exist yet,
- reloads Hyprland.

Every config file the installer touches is backed up first, as
`<file>.bak.<unix-timestamp>`. Installing is idempotent: running it again
only touches what actually changed.

## First run

Open one or more Dofus clients. Each one is placed on its own workspace
(11, 12, 13, ...) without stealing focus, and shows up in the bar as a
generic icon. **Right click a client's icon** to label it: a popup lists the
19 classes (icon + name) plus "Clear class"; picking one labels the account's
currently active character (or the account itself, if no character has been
resolved for it yet). Left click still just focuses that client's workspace.

The icon's tooltip on an unlabelled client says "`<label>` - right click to
choose the class", so first-run discoverability doesn't depend on reading
this file.

The same thing can be done from the terminal:

```sh
$ dofustabs status
SLOT  WORKSPACE   ACCOUNT        NAME             CLASS        CHARACTER  PID
1     11          190231219      Account 190231219              a2902bb7  245681
  -> ACTIVE character not labelled yet: dofustabs character set --account 190231219 --name <name> --class <class>

$ dofustabs character set --account 190231219 --name Anabel --class feca
updated character a2902bb789f8805dd2396ba8c89c906a for account 190231219
```

`--class` accepts aliases, any case, and the French/English name too
(`iop`, `Iop`, `Yopuka` all resolve to the canonical `Yopuka`). The bar and
`dofustabs status` update immediately; no reload needed. `--clear-name` /
`--clear-class` unset a field again (on `character set` and `accounts set`).

### How character detection works, and its honest limit

**The game never tells this tool a character's name or class.** On Linux
every client window is titled exactly `Dofus`, with no character info in it
(verified live; see docs/WORKSPACES.md). So the class has to be told once,
per character, the way described above — there is no way around asking.

What this tool *can* detect automatically is **which character is currently
active** in each running client, so that the name/class you set follows the
right character across logouts and re-logins instead of staying pinned to
whichever character happened to be active when you labelled the account.
Every ~10 seconds (and right after a client opens or closes), `dofustabs
identify` looks at each client's per-character save file under
`~/.config/unity3d/Ankama/Dofus/<RELEASE>/Accounts/<account>/Characters/<hash>/`
and picks whichever one was written to most recently, but only if that is
*after* the client itself started — so a character last played in a previous
session is never mistaken for the one currently active. `dofustabs character
set` (with no `--character`) and the bar picker both target this
automatically-resolved active character.

### Primary class: always slot 1

If you have one character you use to lead groups (a support/enhancer class
you want in a predictable spot regardless of which accounts happen to be
running), set it once:

```sh
dofustabs primary set selotrop
```

From then on, whichever running client's *active* character has that class
always gets slot 1 (workspace 11), overriding any other account's own
`--slot` preference for slot 1 specifically (their preference for every
other slot is unaffected). If two running clients both have the primary
class at once, the lowest account id gets slot 1 and the other is treated as
having no preference at all. `dofustabs primary clear` / `dofustabs primary
show` manage and inspect it.

Optionally pin an account to a slot so it always lands on the same
workspace:

```sh
dofustabs accounts set 190231219 --slot 1
```

## Keys

| Key | From the desktop | From a game workspace |
| --- | --- | --- |
| F1 | Last game workspace used | Previous occupied game workspace |
| F2 | Last game workspace used | Next occupied game workspace |
| F3 | Nothing | Last desktop workspace used |

The three keys are only bound while at least one client is running.

Optional: if your personal Hyprland config uses `e+1`/`e-1` scroll binds for
desktop workspace navigation, point them at `dofustabs.desktop_step(1)` /
`dofustabs.desktop_step(-1)` instead of the relative-workspace dispatcher, so
scrolling never walks into a game workspace:

```lua
-- in your own bindings.lua, replacing the relative-workspace bind:
hl.bind("SUPER, mouse_down", function() dofustabs.desktop_step(1) end)
hl.bind("SUPER, mouse_up", function() dofustabs.desktop_step(-1) end)
```

`dofustabs install`/`uninstall` never edit `bindings.lua` themselves.

## Commands

```
dofustabs accounts list
dofustabs accounts set <account-id> [--name <text>] [--class <class>] [--slot <1-9>] [--clear-name] [--clear-class]
dofustabs accounts remove <account-id>
dofustabs character set --account <id> [--character <hash>] [--name <text>] [--class <class>] [--clear-name] [--clear-class]
dofustabs character list
dofustabs character remove --account <id> --character <hash>
dofustabs character from-title --account <id> --title <title>   # internal; used by the Hyprland module
dofustabs primary set <class> | clear | show
dofustabs identify [--dry-run]   # internal; polled automatically every ~10s
dofustabs status
dofustabs doctor
dofustabs install
dofustabs uninstall
dofustabs launch
dofustabs quit [--keep-launcher] [--dry-run]
```

`dofustabs quit` closes every running game client (asking the compositor to
close the window first, then escalating to `SIGTERM` and finally `SIGKILL`
for any client that does not exit) and, unless `--keep-launcher` is given,
the Ankama launcher too. The bar's cog button exposes it as "Close all
accounts and launcher" (a second click within 4 seconds confirms it).

## Removed

The background CPU limit (a per-client `systemd` `CPUQuota` applied while a
client's workspace was not shown on any monitor) was removed: capping CPU
slowed the game logic of background accounts, not only their rendering.

## Troubleshooting

Run `dofustabs doctor`. It checks Hyprland reachability, whether the module
and widget on disk match this binary's embedded copies, the loader block, the
bar layout, `hyprctl configerrors`, the icons, the state file, and the
F1/F2/F3 binds (present iff a client is running). Exits non-zero if any
check fails.

## Uninstall

```sh
./install.sh --uninstall
```

Reverses everything `install` did (module, loader block, widget, bar entry,
icons) and removes the `dofustabs` binary. `accounts.json` is kept.
