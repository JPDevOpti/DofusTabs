-- DofusTabs Hyprland module.
--
-- Places every Dofus game window on its own workspace (11-19), tracks which
-- account/class occupies each slot, writes a state file for the bar widget,
-- and binds F1/F2/F3 to switch between game clients and back to the desktop.
--
-- Loaded (via a guarded dofile) from ~/.config/hypr/hyprland.lua. Hyprland's
-- Omarchy bootstrap clears package.loaded for "hypr.*" modules on every
-- `hyprctl reload` and re-evaluates them, so this file's top level runs again
-- on every reload. To stay idempotent across reloads without restarting the
-- Hyprland process, all state that must survive a reload is kept on a single
-- global table (`_G.__dofustabs_runtime`), and every event subscription /
-- keybind created on a previous load is explicitly removed before new ones
-- are created.
--
-- The event-wiring / window-moving section at the bottom only runs when a
-- global `hl` (the Hyprland Lua API) is present, so this file can also be
-- `dofile`d by a plain `lua` interpreter (see `hyprland/selfcheck.lua`) to
-- exercise its pure functions (the primary-class slot rule, the shell
-- escaper) without a running compositor.
--
-- See docs/WORKSPACES.md for the full design.

local HOME = os.getenv("HOME") or ""
local ICONS_DIR = HOME .. "/.local/share/dofustabs/icons"
local ACCOUNTS_FILE = HOME .. "/.config/dofustabs/accounts.lua"
local RUNTIME_DIR = (os.getenv("XDG_RUNTIME_DIR") or (HOME .. "/.cache")) .. "/dofustabs"
-- Runtime state, not configuration (rewritten every ~10s by the periodic
-- identify poll), so it lives next to state.json rather than under
-- ~/.config/dofustabs/.
local IDENTITY_FILE = RUNTIME_DIR .. "/identity.lua"
local STATE_FILE = RUNTIME_DIR .. "/state.json"
local LOG_FILE = HOME .. "/.cache/dofustabs.log"

local DOFUSTABS_BIN = HOME .. "/.local/bin/dofustabs"

local FIRST_SLOT_WORKSPACE = 11
local LAST_SLOT_WORKSPACE = 19
local MAX_SLOT = LAST_SLOT_WORKSPACE - FIRST_SLOT_WORKSPACE + 1 -- 9

-- How often `dofustabs identify` is polled while at least one client is
-- running (milliseconds). See "Periodic identify" below.
local IDENTIFY_INTERVAL_MS = 10000

-- MX Master 3S side buttons (kernel BTN_SIDE / BTN_EXTRA, evdev codes 275
-- and 276): back = previous game client, forward = next game client. Bound
-- only while a client is tracked (see update_keybinds), so they behave as
-- ordinary back/forward everywhere else.
local MOUSE_BACK = "mouse:275"
local MOUSE_FORWARD = "mouse:276"

-- ── Small utilities ─────────────────────────────────────────────────────────

local function log(message)
  local ok = pcall(function()
    local f = io.open(LOG_FILE, "a")
    if not f then
      return
    end
    f:write(os.date("%Y-%m-%d %H:%M:%S") .. " " .. tostring(message) .. "\n")
    f:close()
  end)
  return ok
end

-- Wraps a callback so a Lua error inside it can never break the compositor
-- config: it is caught and logged instead of propagating.
local function safe(fn)
  return function(...)
    local ok, err = pcall(fn, ...)
    if not ok then
      log("ERROR: " .. tostring(err))
    end
  end
end

-- Single-quotes `s` for safe interpolation into a shell command line: only a
-- literal `'` needs escaping (closed, an escaped quote, reopened), and
-- everything else a shell would otherwise treat specially inside double
-- quotes or bare words (backticks, `$(...)`, `;`, newlines, `*`, ...) is
-- inert inside single quotes. This is the only path window titles (untrusted
-- input: the game client picks its own title) are ever passed through to
-- `hl.exec_cmd`; they are never interpolated into a Lua chunk.
local function shell_single_quote(s)
  s = tostring(s)
  return "'" .. s:gsub("'", "'\\''") .. "'"
end

local function mkdir_p(path)
  os.execute("mkdir -p " .. shell_single_quote(path))
end

local function file_exists(path)
  local f = io.open(path, "rb")
  if f then
    f:close()
    return true
  end
  return false
end

local function is_workspace_in_range(id, lo, hi)
  return type(id) == "number" and id >= lo and id <= hi
end

local function is_desktop_workspace(id)
  return is_workspace_in_range(id, 1, 10)
end

local function is_game_workspace(id)
  return is_workspace_in_range(id, FIRST_SLOT_WORKSPACE, LAST_SLOT_WORKSPACE)
end

local function slot_to_workspace(slot)
  return FIRST_SLOT_WORKSPACE - 1 + slot
end

local function workspace_to_slot(id)
  if not is_game_workspace(id) then
    return nil
  end
  return id - (FIRST_SLOT_WORKSPACE - 1)
end

-- ── Persistent state (survives a config reload, not a process restart) ─────

_G.__dofustabs_runtime = _G.__dofustabs_runtime or {
  slots = {}, -- [slot] = { pid, address, class, account, name, icon_class, character }
  subs = {}, -- [event_name] = HL.EventSubscription
  keybinds = {}, -- [key] = HL.Keybind
  keys_bound = false,
  last_desktop_ws = 1,
  last_game_ws = nil,
  identify_timer = nil, -- HL.Timer, created once and reused across reloads
}

local R = _G.__dofustabs_runtime

-- ── Class / account detection ───────────────────────────────────────────────

local function is_game_class(class)
  if type(class) ~= "string" or class == "" then
    return false
  end
  local lower = class:lower()
  return lower:sub(1, 5) == "dofus" and lower ~= "dofustabs"
end

local function read_account_id(pid)
  if not pid then
    return ""
  end

  local ok, id = pcall(function()
    local f = io.open("/proc/" .. tostring(pid) .. "/cmdline", "rb")
    if not f then
      return ""
    end
    local data = f:read("*a") or ""
    f:close()
    return data:match("dofus%.(%d+)%.log") or ""
  end)

  if ok and id then
    return id
  end
  return ""
end

-- Drops preferred slots that cannot be honoured: non-integer or out of range
-- values, and duplicates. The CLI rejects these, but accounts.lua can be
-- edited by hand. Among accounts claiming the same slot the lowest account id
-- keeps it, so the outcome is the same on every load and windows never swap
-- back and forth between reloads.
local function sanitize_accounts(tbl, max_slot)
  local ids = {}
  for id, entry in pairs(tbl) do
    if type(entry) == "table" then
      ids[#ids + 1] = tostring(id)
    else
      tbl[id] = nil
    end
  end
  table.sort(ids)

  local claimed = {}
  for _, id in ipairs(ids) do
    local entry = tbl[id] or tbl[tonumber(id)]
    local slot = entry.slot
    if slot ~= nil then
      local valid = type(slot) == "number" and slot == math.floor(slot) and slot >= 1 and slot <= max_slot
      if not valid or claimed[slot] then
        entry.slot = nil
      else
        claimed[slot] = true
      end
    end
  end
  return tbl
end

-- Reads accounts.lua and returns `accounts, primary_class`. Tolerates both
-- shapes: the current one, `{ primary_class = "...", accounts = { [id] =
-- {...} } }` (detected by the presence of the `accounts` key), and the old
-- flat one, `{ [id] = {...} }`, so a stale generated file from before this
-- change never breaks placement. A missing or unreadable file yields no
-- accounts and no primary class, same as before.
local function load_accounts()
  local ok, tbl = pcall(dofile, ACCOUNTS_FILE)
  if not ok or type(tbl) ~= "table" then
    return {}, nil
  end

  local max_slot = LAST_SLOT_WORKSPACE - FIRST_SLOT_WORKSPACE + 1
  if type(tbl.accounts) == "table" then
    local primary_class = tbl.primary_class
    if type(primary_class) ~= "string" or primary_class == "" then
      primary_class = nil
    end
    return sanitize_accounts(tbl.accounts, max_slot), primary_class
  end
  return sanitize_accounts(tbl, max_slot), nil
end

-- Reads identity.lua (written by `dofustabs identify`): `{ [account_id] =
-- { character = "<hash>", release = "..." } }`. Missing/unreadable/malformed
-- file means "nothing resolved yet", not an error.
local function load_identity()
  local ok, tbl = pcall(dofile, IDENTITY_FILE)
  if ok and type(tbl) == "table" then
    return tbl
  end
  return {}
end

local function icon_path_for(class)
  if type(class) == "string" and class ~= "" then
    local candidate = ICONS_DIR .. "/" .. class .. ".png"
    if file_exists(candidate) then
      return candidate
    end
  end
  return ICONS_DIR .. "/default.png"
end

-- Resolves the display name, class and active-character hash for `account_id`
-- (in slot `slot`, only used for the "Dofus <slot>" fallback name):
-- character-level values win, then account-level ones, then the generic
-- fallbacks. A value is only used when it is a non-empty string, so a
-- present-but-blank field falls through like an absent one.
local function resolve_name_class_character(account_id, slot, accounts, identity)
  local hash = (account_id ~= "" and identity[account_id] and identity[account_id].character) or ""
  if type(hash) ~= "string" then
    hash = ""
  end
  local acc = account_id ~= "" and accounts[account_id] or nil
  local char_entry = (hash ~= "" and acc and type(acc.characters) == "table" and acc.characters[hash]) or nil

  local function non_empty(v)
    return type(v) == "string" and v ~= "" and v or nil
  end

  local name = non_empty(char_entry and char_entry.name)
    or non_empty(acc and acc.name)
    or (account_id ~= "" and ("Account " .. account_id))
    or ("Dofus " .. tostring(slot))
  local class = non_empty(char_entry and char_entry.class) or non_empty(acc and acc.class) or ""

  return name, class, hash
end

-- ── Primary-class slot override (mirrors dofustabs_core::slot in Rust) ──────
--
-- This is a straight Lua port of `dofustabs_core::slot::effective_preferred_slots`
-- (crates/core/src/slot.rs): given the running clients (with their effective
-- class) and each account's own preferred slot, it returns each running
-- account's *effective* preferred slot after the primary-class override.
-- `hyprland/selfcheck.lua` runs the same fixture cases as the Rust unit
-- tests against this function so the two stay in sync; if you change the
-- rule, update both.
local function effective_preferred_slots(running, slot_prefs, primary_class)
  local result = {}
  for _, r in ipairs(running) do
    result[r.account_id] = slot_prefs[r.account_id]
  end

  if not primary_class or primary_class == "" then
    return result
  end

  local seen = {}
  local holders = {}
  for _, r in ipairs(running) do
    if
      type(r.effective_class) == "string"
      and r.effective_class ~= ""
      and r.effective_class:lower() == primary_class:lower()
      and not seen[r.account_id]
    then
      seen[r.account_id] = true
      holders[#holders + 1] = r.account_id
    end
  end
  if #holders == 0 then
    return result
  end
  table.sort(holders)

  local winner = holders[1]
  result[winner] = 1

  for account_id, slot in pairs(result) do
    if account_id ~= winner and slot == 1 then
      result[account_id] = nil
    end
  end
  for i = 2, #holders do
    result[holders[i]] = nil
  end

  return result
end

-- ── Slot bookkeeping ─────────────────────────────────────────────────────────

local function find_slot_by_address(address)
  for slot, entry in pairs(R.slots) do
    if entry.address == address then
      return slot
    end
  end
  return nil
end

-- Every slot any account prefers (per `prefs`, a plain account_id -> slot
-- map), whether or not that account is currently running. These are
-- reserved: an unconfigured/no-preference client must not take them while a
-- non-reserved slot is free, so opening an unrelated client first can never
-- squat on a main account's pinned slot.
local function reserved_slots_from_prefs(prefs, max_slot)
  local reserved = {}
  for _, slot in pairs(prefs) do
    if type(slot) == "number" and slot >= 1 and slot <= max_slot then
      reserved[slot] = true
    end
  end
  return reserved
end

-- Pure slot-selection rule, kept as one small function so it can be reasoned
-- about (and tested) on its own:
--   `occupied`    slot -> account_id, for every slot currently held.
--   `prefs`       account_id -> preferred slot (already includes the
--                 primary-class override for running accounts; see
--                 `merged_slot_prefs`).
--   `account_id`  the account being placed ("" when unknown).
--   `max_slot`    highest valid slot number.
--
-- Rule: an account with a preferred slot always gets that slot (the caller
-- is responsible for freeing it first if occupied by someone else). Anyone
-- else takes the lowest free slot that is not reserved for a different
-- account; only once every non-reserved slot is taken does a reserved-but-
-- free slot become eligible, rather than refusing placement.
local function choose_slot(occupied, prefs, account_id, max_slot)
  local preferred = account_id ~= "" and prefs[account_id]
  if type(preferred) == "number" and preferred >= 1 and preferred <= max_slot then
    return preferred
  end

  local reserved = reserved_slots_from_prefs(prefs, max_slot)
  for slot = 1, max_slot do
    if not occupied[slot] and not reserved[slot] then
      return slot
    end
  end
  for slot = 1, max_slot do
    if not occupied[slot] then
      return slot
    end
  end
  return nil
end

-- slot -> account_id for every slot currently tracked, optionally excluding
-- one slot (used while displacing its occupant, before it has a new home).
local function occupied_slots(exclude_slot)
  local occupied = {}
  for slot, entry in pairs(R.slots) do
    if slot ~= exclude_slot then
      occupied[slot] = entry.account
    end
  end
  return occupied
end

-- account_id -> own preferred slot, straight from accounts.lua.
local function slot_prefs_from_accounts(accounts)
  local prefs = {}
  for id, acc in pairs(accounts) do
    if type(acc) == "table" and type(acc.slot) == "number" then
      prefs[id] = acc.slot
    end
  end
  return prefs
end

-- Every account currently tracked in R.slots (deduplicated), as the
-- `running` list `effective_preferred_slots` expects, plus an optional
-- extra entry for a window not yet tracked (the one about to be placed).
local function build_running_list(accounts, identity, extra)
  local list = {}
  local seen = {}
  for _, entry in pairs(R.slots) do
    local account_id = entry.account or ""
    if account_id ~= "" and not seen[account_id] then
      seen[account_id] = true
      local _, class = resolve_name_class_character(account_id, 0, accounts, identity)
      list[#list + 1] = { account_id = account_id, effective_class = class ~= "" and class or nil }
    end
  end
  if extra and extra.account_id ~= "" and not seen[extra.account_id] then
    list[#list + 1] = extra
  end
  return list
end

-- account_id -> effective preferred slot: each running account's own
-- preference from accounts.lua, then the primary-class override on top. A
-- non-running account's own preference is preserved as-is (it still
-- reserves its slot; see `reserved_slots_from_prefs`).
local function merged_slot_prefs(accounts, running, primary_class)
  local raw = slot_prefs_from_accounts(accounts)
  local overridden = effective_preferred_slots(running, raw, primary_class)
  local merged = {}
  for id, slot in pairs(raw) do
    merged[id] = slot
  end
  for _, r in ipairs(running) do
    merged[r.account_id] = overridden[r.account_id]
  end
  return merged
end

local function build_slot_entry(win, slot, accounts, identity)
  local account_id = read_account_id(win.pid)
  local name, class, hash = resolve_name_class_character(account_id, slot, accounts, identity)

  return {
    pid = win.pid,
    address = win.address,
    class = class,
    account = account_id,
    name = name,
    icon_class = class,
    character = hash,
  }
end

-- Moves a window into its slot's workspace without stealing focus.
local function move_window_to_slot(win, slot)
  local ws_id = slot_to_workspace(slot)

  if win.group then
    pcall(function()
      win.group:remove(win)
    end)
  end

  pcall(function()
    hl.dispatch(hl.dsp.window.move({
      workspace = tostring(ws_id),
      follow = false,
      window = "address:" .. win.address,
    }))
  end)
end

-- ── State file (consumed by the bar widget) ─────────────────────────────────

local function json_escape(s)
  s = tostring(s)
  local out = {}
  for i = 1, #s do
    local c = s:sub(i, i)
    local byte = c:byte()
    if c == '"' then
      out[#out + 1] = '\\"'
    elseif c == "\\" then
      out[#out + 1] = "\\\\"
    elseif c == "\n" then
      out[#out + 1] = "\\n"
    elseif c == "\r" then
      out[#out + 1] = "\\r"
    elseif c == "\t" then
      out[#out + 1] = "\\t"
    elseif byte < 0x20 then
      out[#out + 1] = string.format("\\u%04x", byte)
    else
      out[#out + 1] = c
    end
  end
  return table.concat(out)
end

local function write_state()
  mkdir_p(RUNTIME_DIR)

  local slots = {}
  for slot = 1, MAX_SLOT do
    if R.slots[slot] then
      table.insert(slots, slot)
    end
  end
  table.sort(slots)

  local parts = {}
  for _, slot in ipairs(slots) do
    local e = R.slots[slot]
    local icon = icon_path_for(e.icon_class)
    local row = string.format(
      '{"slot":%d,"workspace":%d,"account":"%s","name":"%s","class":"%s","icon":"%s","pid":%d,"address":"%s","character":"%s"}',
      slot,
      slot_to_workspace(slot),
      json_escape(e.account or ""),
      json_escape(e.name or ""),
      json_escape(e.class or ""),
      json_escape(icon),
      tonumber(e.pid) or 0,
      json_escape(e.address or ""),
      json_escape(e.character or "")
    )
    table.insert(parts, row)
  end

  local json = '{"version":1,"clients":[' .. table.concat(parts, ",") .. "]}"

  local tmp_path = STATE_FILE .. ".tmp"
  local ok = pcall(function()
    local f = io.open(tmp_path, "w")
    if not f then
      error("cannot open " .. tmp_path)
    end
    f:write(json)
    f:close()
    local renamed, rename_err = os.rename(tmp_path, STATE_FILE)
    if not renamed then
      error("cannot rename state file: " .. tostring(rename_err))
    end
  end)

  if not ok then
    log("ERROR: failed to write state file")
  end
end

-- ── Keybinds (F1/F2/F3) ──────────────────────────────────────────────────────

local function occupied_game_workspaces()
  local ids = {}
  for slot = 1, MAX_SLOT do
    if R.slots[slot] then
      table.insert(ids, slot_to_workspace(slot))
    end
  end
  table.sort(ids)
  return ids
end

local function focus_workspace(id)
  pcall(function()
    hl.dispatch(hl.dsp.focus({ workspace = tostring(id) }))
  end)
end

local function current_workspace_id()
  local ok, ws = pcall(hl.get_active_workspace)
  if ok and ws then
    return ws.id
  end
  return nil
end

local function step_game_workspace(delta)
  local ids = occupied_game_workspaces()
  if #ids == 0 then
    return
  end

  local current = current_workspace_id()
  local index = nil
  if current then
    for i, id in ipairs(ids) do
      if id == current then
        index = i
        break
      end
    end
  end

  if not index then
    -- Not currently on an occupied game workspace: jump to the last one used,
    -- or the first available one.
    focus_workspace(R.last_game_ws and workspace_to_slot(R.last_game_ws) and R.last_game_ws or ids[1])
    return
  end

  local next_index = ((index - 1 + delta) % #ids) + 1
  focus_workspace(ids[next_index])
end

local function goto_game_or_last(delta)
  local current = current_workspace_id()
  if current and is_desktop_workspace(current) then
    local ids = occupied_game_workspaces()
    if #ids == 0 then
      return
    end
    if R.last_game_ws then
      local still_occupied = false
      for _, id in ipairs(ids) do
        if id == R.last_game_ws then
          still_occupied = true
          break
        end
      end
      focus_workspace(still_occupied and R.last_game_ws or ids[1])
    else
      focus_workspace(ids[1])
    end
    return
  end

  step_game_workspace(delta)
end

local function go_home()
  local current = current_workspace_id()
  if current and is_game_workspace(current) then
    focus_workspace(R.last_desktop_ws or 1)
  end
  -- From the desktop, F3 has no defined behavior.
end

local function goto_slot(n)
  n = tonumber(n)
  if not n or n < 1 or n > MAX_SLOT then
    return
  end
  if R.slots[n] then
    focus_workspace(slot_to_workspace(n))
  end
end

local function update_keybinds()
  local has_clients = false
  for slot = 1, MAX_SLOT do
    if R.slots[slot] then
      has_clients = true
      break
    end
  end

  if has_clients and not R.keys_bound then
    pcall(hl.unbind, "F1")
    pcall(hl.unbind, "F2")
    pcall(hl.unbind, "F3")

    R.keybinds.F1 = hl.bind(
      "F1",
      safe(function()
        goto_game_or_last(-1)
      end),
      { description = "DofusTabs: previous game client" }
    )
    R.keybinds.F2 = hl.bind(
      "F2",
      safe(function()
        goto_game_or_last(1)
      end),
      { description = "DofusTabs: next game client" }
    )
    R.keybinds.F3 = hl.bind(
      "F3",
      safe(function()
        go_home()
      end),
      { description = "DofusTabs: back to desktop" }
    )
    -- Mouse side buttons: same behavior as F1/F2, bound only while a client
    -- is tracked so back/forward keep working normally in every app
    -- otherwise. Never unbound by key name (mouse buttons keep every other
    -- bind on the key); only ever removed through the handle below.
    R.keybinds[MOUSE_BACK] = hl.bind(
      MOUSE_BACK,
      safe(function()
        goto_game_or_last(-1)
      end),
      { description = "DofusTabs: previous game client (mouse back)" }
    )
    R.keybinds[MOUSE_FORWARD] = hl.bind(
      MOUSE_FORWARD,
      safe(function()
        goto_game_or_last(1)
      end),
      { description = "DofusTabs: next game client (mouse forward)" }
    )
    R.keys_bound = true
  elseif not has_clients and R.keys_bound then
    for _, key in ipairs({ "F1", "F2", "F3" }) do
      local kb = R.keybinds[key]
      if kb then
        pcall(function()
          kb:remove()
        end)
      end
      pcall(hl.unbind, key)
      R.keybinds[key] = nil
    end
    for _, key in ipairs({ MOUSE_BACK, MOUSE_FORWARD }) do
      local kb = R.keybinds[key]
      if kb then
        pcall(function()
          kb:remove()
        end)
      end
      R.keybinds[key] = nil
    end
    R.keys_bound = false
  end
end

-- Resolved as an absolute path first (matches how the CLI is installed by
-- install.sh) with a PATH fallback for a manual/dev binary. `hl.exec_cmd`
-- inherits Hyprland's own environment (its PATH, HOME, XDG_RUNTIME_DIR, etc.,
-- the same environment `hyprctl` calls run under), so both forms resolve the
-- same way `hyprctl` would.
local function dofustabs_cmd()
  if file_exists(DOFUSTABS_BIN) then
    return DOFUSTABS_BIN
  end
  return "dofustabs"
end

-- ── Active-character identification ─────────────────────────────────────────
--
-- `dofustabs identify` resolves which character is active in each running
-- client and writes identity.json / identity.lua; this module only ever
-- triggers it and reads the result back. `refresh()` (called by the CLI
-- after `accounts set`/`character set`/`primary set`) must never trigger
-- identify itself, so it stays a one-way, loop-free relationship.
local function run_identify()
  pcall(function()
    hl.exec_cmd(dofustabs_cmd() .. " identify")
  end)
end

local function has_any_client()
  for slot = 1, MAX_SLOT do
    if R.slots[slot] then
      return true
    end
  end
  return false
end

-- Starts (or keeps) a 10 s repeating timer that polls `dofustabs identify`
-- while at least one client is tracked, and disables it otherwise. The timer
-- handle is kept on `R` (the runtime table that survives a config reload),
-- so a reload never creates a second timer alongside a still-live one: this
-- function only creates one when `R.identify_timer` is not already set.
--
-- Verified live: `hl.timer(fn, { timeout = ms, type = "repeat" })` returns an
-- `HL.Timer` and fires reliably; the only way to stop it is
-- `timer:set_enabled(false)` (no `remove`/`cancel`/`stop` method exists).
local function ensure_identify_timer()
  if has_any_client() then
    if not R.identify_timer then
      local ok, timer_or_err = pcall(hl.timer, safe(run_identify), {
        timeout = IDENTIFY_INTERVAL_MS,
        type = "repeat",
      })
      if ok then
        R.identify_timer = timer_or_err
      else
        log("ERROR: could not start the identify timer: " .. tostring(timer_or_err))
      end
    end
  elseif R.identify_timer then
    pcall(function()
      R.identify_timer:set_enabled(false)
    end)
    R.identify_timer = nil
  end
end

-- ── Placement ────────────────────────────────────────────────────────────────

-- Moves whatever currently sits in `slot` to some other slot chosen by
-- `choose_slot` (excluding `slot` itself from the occupied set while
-- deciding), using `follow = false` so the user's focus is unaffected unless
-- the displaced window happened to be focused. No-op if the slot is empty.
local function displace_occupant(slot, prefs)
  local occupant = R.slots[slot]
  if not occupant then
    return
  end
  local occupied = occupied_slots(slot)
  local new_slot = choose_slot(occupied, prefs, occupant.account or "", MAX_SLOT)
  if not new_slot then
    log("WARN: no slot available to displace occupant of slot " .. tostring(slot))
    return
  end
  move_window_to_slot({ address = occupant.address }, new_slot)
  R.slots[new_slot] = occupant
  R.slots[slot] = nil
end

local function assign_and_place(win)
  if find_slot_by_address(win.address) then
    return -- already tracked (e.g. duplicate open/open_early firing)
  end

  local accounts, primary_class = load_accounts()
  local identity = load_identity()
  local account_id = read_account_id(win.pid)
  local _, effective_class = resolve_name_class_character(account_id, 0, accounts, identity)
  local running = build_running_list(
    accounts,
    identity,
    { account_id = account_id, effective_class = effective_class ~= "" and effective_class or nil }
  )
  local prefs = merged_slot_prefs(accounts, running, primary_class)
  local preferred = account_id ~= "" and prefs[account_id]

  local slot
  if type(preferred) == "number" and preferred >= 1 and preferred <= MAX_SLOT then
    local occupant = R.slots[preferred]
    if occupant and occupant.account ~= account_id then
      displace_occupant(preferred, prefs)
    end
    slot = preferred
  else
    slot = choose_slot(occupied_slots(nil), prefs, account_id, MAX_SLOT)
  end

  if not slot then
    log("WARN: no free slot for window " .. tostring(win.address))
    return
  end

  R.slots[slot] = build_slot_entry(win, slot, accounts, identity)
  move_window_to_slot(win, slot)
  update_keybinds()
  write_state()
  ensure_identify_timer()
  run_identify()
end

local function release_window(win)
  local slot = find_slot_by_address(win.address)
  if not slot then
    return
  end
  R.slots[slot] = nil
  update_keybinds()
  write_state()
  ensure_identify_timer()
  run_identify()
end

-- Adopts windows already sitting on 11-19 (startup / reload), then places any
-- stray game window found elsewhere.
local function adopt_and_place_all()
  local ok, windows = pcall(hl.get_windows)
  if not ok or not windows then
    return
  end

  local accounts, _ = load_accounts()
  local identity = load_identity()

  -- Pass 1: adopt windows already correctly placed, without moving them.
  for _, win in ipairs(windows) do
    if is_game_class(win.class) and win.workspace and is_game_workspace(win.workspace.id) then
      local slot = workspace_to_slot(win.workspace.id)
      if slot and not R.slots[slot] and not find_slot_by_address(win.address) then
        R.slots[slot] = build_slot_entry(win, slot, accounts, identity)
      end
    end
  end

  -- Pass 2: place any game window sitting elsewhere.
  for _, win in ipairs(windows) do
    if is_game_class(win.class) and not find_slot_by_address(win.address) then
      assign_and_place(win)
    end
  end

  update_keybinds()
  write_state()
end

-- Moves every tracked client that is not on its account's effective
-- preferred slot there, displacing whatever else occupies that slot (rule 2
-- in docs/WORKSPACES.md "Slot rules", plus the primary-class override on
-- top). Bounded to `MAX_SLOT` passes: each pass fixes at least one
-- misplacement or stops, and there are at most MAX_SLOT clients, so this
-- always terminates without an unbounded loop. Uses `follow = false` moves
-- throughout, so the user's focused workspace is never changed by
-- reconciliation itself. Idempotent: a second call with the same accounts
-- and identity data makes zero moves, since every account already sitting on
-- its preferred slot is skipped by the `preferred ~= slot` guard below.
local function reconcile_slots(accounts, identity, primary_class)
  local running = build_running_list(accounts, identity, nil)
  local prefs = merged_slot_prefs(accounts, running, primary_class)

  for _ = 1, MAX_SLOT do
    local moved = false
    for slot, entry in pairs(R.slots) do
      local account_id = entry.account or ""
      local preferred = account_id ~= "" and prefs[account_id]
      if
        type(preferred) == "number"
        and preferred >= 1
        and preferred <= MAX_SLOT
        and preferred ~= slot
      then
        local occupant = R.slots[preferred]
        if occupant and occupant.account ~= account_id then
          displace_occupant(preferred, prefs)
        end
        if not R.slots[preferred] or R.slots[preferred].account == account_id then
          move_window_to_slot({ address = entry.address }, preferred)
          R.slots[preferred] = entry
          R.slots[slot] = nil
          moved = true
          break -- table just mutated mid-iteration; restart the scan
        end
      end
    end
    if not moved then
      break
    end
  end
end

-- Re-reads accounts.lua/identity.lua, refreshes name/class/icon/character for
-- every tracked slot, moves any client that is not on its effective
-- preferred slot there (displacing the occupant per rule 2), and rewrites
-- state.json. Called by the CLI after `accounts set`/`character
-- set`/`primary set`/`remove` so the bar and window placement update
-- immediately instead of waiting for the next window event or config
-- reload. Never triggers `identify` itself.
local function refresh_from_accounts()
  local accounts, primary_class = load_accounts()
  local identity = load_identity()
  reconcile_slots(accounts, identity, primary_class)
  for slot, entry in pairs(R.slots) do
    local account_id = entry.account or ""
    local name, class, hash = resolve_name_class_character(account_id, slot, accounts, identity)
    entry.name = name
    entry.class = class
    entry.icon_class = class
    entry.character = hash
  end
  write_state()
end

-- ── Event wiring (idempotent across reloads) ────────────────────────────────

local function subscribe(event_name, handler)
  local previous = R.subs[event_name]
  if previous then
    pcall(function()
      previous:remove()
    end)
  end
  R.subs[event_name] = hl.on(event_name, safe(handler))
end

local function on_window_open(win)
  if not win or not is_game_class(win.class) then
    return
  end
  assign_and_place(win)
end

local function on_window_gone(win)
  if not win or not is_game_class(win.class) then
    return
  end
  release_window(win)
end

local function on_workspace_active(ws)
  if not ws or type(ws.id) ~= "number" then
    return
  end
  if is_desktop_workspace(ws.id) then
    R.last_desktop_ws = ws.id
  elseif is_game_workspace(ws.id) then
    R.last_game_ws = ws.id
  end
end

-- Part C: future-proofing against a client whose title one day does carry
-- the character name/class (verified today it never does, on Linux; see
-- docs/WORKSPACES.md). `title` is untrusted (the game client picks it), so
-- it is only ever passed through `shell_single_quote` into `hl.exec_cmd`,
-- never interpolated into a Lua chunk.
local function on_window_title(win)
  if not win or not is_game_class(win.class) then
    return
  end
  local title = win.title
  if type(title) ~= "string" or title == "" or title == "Dofus" then
    return
  end
  local account_id = read_account_id(win.pid)
  if account_id == "" then
    return
  end
  local cmd = dofustabs_cmd()
    .. " character from-title --account "
    .. shell_single_quote(account_id)
    .. " --title "
    .. shell_single_quote(title)
  pcall(function()
    hl.exec_cmd(cmd)
  end)
end

-- The game asks for attention when a character's turn starts (its "focus
-- the application" option). Each client lives on its own workspace, so
-- honour the request by switching to that workspace. Only while the user is
-- already on a game workspace: a turn must not pull them out of the desktop.
local function on_window_urgent(win)
  if not win or not is_game_class(win.class) then
    return
  end
  local slot = find_slot_by_address(win.address)
  local current = current_workspace_id()
  log("attention requested by slot " .. tostring(slot) .. " while on workspace " .. tostring(current))
  if not slot or not current or not is_game_workspace(current) then
    return
  end
  local target = slot_to_workspace(slot)
  if target ~= current then
    focus_workspace(target)
  end
end

if _G.hl then
  subscribe("window.urgent", on_window_urgent)
  subscribe("window.open_early", on_window_open)
  subscribe("window.open", on_window_open) -- defensive fallback, deduped by address
  subscribe("window.close", on_window_gone)
  subscribe("window.destroy", on_window_gone)
  subscribe("workspace.active", on_workspace_active)
  subscribe("window.title", on_window_title)

  mkdir_p(RUNTIME_DIR)
  adopt_and_place_all()
  -- A config reload just adopted whatever is currently on 11-19 as-is; apply
  -- the preferred-slot reconciliation on top so accounts.lua changes made
  -- while clients were running take effect on the very next reload too.
  do
    local accounts, primary_class = load_accounts()
    local identity = load_identity()
    reconcile_slots(accounts, identity, primary_class)
  end
  write_state()
  ensure_identify_timer()
  run_identify() -- also trigger identify once right on module load

  -- `hyprctl reload` resets the Lua state entirely (verified: a global set
  -- via one `hyprctl eval` does not survive a reload), so `last_desktop_ws` /
  -- `last_game_ws` can never be recovered from a previous load. Seed them
  -- from whatever is actually focused right now, which is the best
  -- available guess immediately after a reload (the user is still sitting
  -- on the workspace they reloaded from).
  do
    local ok, ws = pcall(hl.get_active_workspace)
    if ok and ws and type(ws.id) == "number" then
      if is_desktop_workspace(ws.id) then
        R.last_desktop_ws = ws.id
      elseif is_game_workspace(ws.id) then
        R.last_game_ws = ws.id
      end
    end
  end
end

-- ── Public API ───────────────────────────────────────────────────────────────

_G.dofustabs = {
  desktop_step = function(delta)
    local ok, err = pcall(function()
      local mon
      local ok_mon, m = pcall(hl.get_active_monitor)
      if ok_mon then
        mon = m
      end

      local ok_ws, workspaces = pcall(hl.get_workspaces)
      if not ok_ws or not workspaces then
        return
      end

      local ids = {}
      for _, ws in ipairs(workspaces) do
        if is_desktop_workspace(ws.id) then
          if not mon or (ws.monitor and ws.monitor.id == mon.id) then
            table.insert(ids, ws.id)
          end
        end
      end
      table.sort(ids)
      if #ids == 0 then
        return
      end

      local current = current_workspace_id()
      local index = nil
      if current then
        for i, id in ipairs(ids) do
          if id == current then
            index = i
            break
          end
        end
      end

      if not index then
        focus_workspace(ids[1])
        return
      end

      local next_index = ((index - 1 + delta) % #ids) + 1
      focus_workspace(ids[next_index])
    end)
    if not ok then
      log("ERROR: desktop_step: " .. tostring(err))
    end
  end,

  goto_slot = function(n)
    local ok, err = pcall(goto_slot, n)
    if not ok then
      log("ERROR: goto_slot: " .. tostring(err))
    end
  end,

  next = function()
    local ok, err = pcall(goto_game_or_last, 1)
    if not ok then
      log("ERROR: next: " .. tostring(err))
    end
  end,

  prev = function()
    local ok, err = pcall(goto_game_or_last, -1)
    if not ok then
      log("ERROR: prev: " .. tostring(err))
    end
  end,

  home = function()
    local ok, err = pcall(go_home)
    if not ok then
      log("ERROR: home: " .. tostring(err))
    end
  end,

  refresh = function()
    local ok, err = pcall(refresh_from_accounts)
    if not ok then
      log("ERROR: refresh: " .. tostring(err))
    end
  end,

  -- Exposed only for `hyprland/selfcheck.lua`: pure functions with no `hl`
  -- dependency, so they can be exercised without a running compositor.
  _internal = {
    effective_preferred_slots = effective_preferred_slots,
    merged_slot_prefs = merged_slot_prefs,
    choose_slot = choose_slot,
    shell_single_quote = shell_single_quote,
    resolve_name_class_character = resolve_name_class_character,
  },
}

log("loaded")
