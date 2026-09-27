-- Plain-`lua` self-check for the pure functions in `dofustabs.lua`: no
-- Hyprland, no `hl` global, no file-system side effects (the module's
-- event-wiring/window-moving section only runs when `_G.hl` is set).
--
-- Run with: lua54 hyprland/selfcheck.lua   (or: lua hyprland/selfcheck.lua)
--
-- Mirrors the fixture cases in crates/core/src/slot.rs's tests
-- (`effective_preferred_slots`) so the Rust reference implementation and
-- this Lua port are checked against the same inputs/outputs. Keep both in
-- sync when the rule changes.

local script_dir = arg[0]:match("(.*/)") or "./"
dofile(script_dir .. "dofustabs.lua")

local internal = _G.dofustabs._internal
assert(internal, "dofustabs.lua did not expose _internal")

local failures = 0

local function eq(a, b)
  return a == b
end

local function check(name, got, want)
  if not eq(got, want) then
    failures = failures + 1
    print(string.format("FAIL %s: got %s, want %s", name, tostring(got), tostring(want)))
  else
    print(string.format("ok   %s", name))
  end
end

-- ── effective_preferred_slots: same cases as crates/core/src/slot.rs ───────

local function client(id, class)
  return { account_id = id, effective_class = class }
end

do
  local running = { client("1", "Feca"), client("2", "Yopuka") }
  local prefs = { ["2"] = 1 }
  local result = internal.effective_preferred_slots(running, prefs, nil)
  check("no_primary_class_configured/1", result["1"], nil)
  check("no_primary_class_configured/2", result["2"], 1)
end

do
  local running = { client("1", "Yopuka"), client("2", "Feca") }
  local prefs = { ["2"] = 1, ["1"] = 3 }
  local result = internal.effective_preferred_slots(running, prefs, "Yopuka")
  check("primary_holder_gets_slot_1/1", result["1"], 1)
  check("primary_holder_overrides_others_pref/2", result["2"], nil)
end

do
  local running = { client("1", "Yopuka"), client("2", "Feca") }
  local prefs = { ["2"] = 4 }
  local result = internal.effective_preferred_slots(running, prefs, "Yopuka")
  check("other_preferences_unaffected", result["2"], 4)
end

do
  local running = { client("200", "Yopuka"), client("100", "Yopuka") }
  local result = internal.effective_preferred_slots(running, {}, "Yopuka")
  check("two_primary_holders_lowest_id_wins/100", result["100"], 1)
  check("two_primary_holders_lowest_id_wins/200", result["200"], nil)
end

do
  local running = { client("1", "Feca") }
  local prefs = { ["1"] = 1 }
  local result = internal.effective_preferred_slots(running, prefs, "Yopuka")
  check("primary_not_running_leaves_prefs_untouched", result["1"], 1)
end

do
  local running = { client("1", "yopuka") }
  local result = internal.effective_preferred_slots(running, {}, "Yopuka")
  check("class_match_is_case_insensitive", result["1"], 1)
end

-- ── Idempotency: a second reconcile pass over the same state moves nothing.
--
-- `reconcile_slots` in dofustabs.lua only ever moves an account whose
-- current slot differs from its effective preferred slot (the
-- `preferred ~= slot` guard). This is a pure re-simulation of that guard
-- over a plain slot->account_id map, run twice: the second pass must be a
-- no-op once the first pass has settled everyone onto their preferred slot.
local function reconcile_pass(occupied, prefs, max_slot)
  local moved = false
  local next_occupied = {}
  for slot, id in pairs(occupied) do
    next_occupied[slot] = id
  end
  for slot, account_id in pairs(occupied) do
    local preferred = account_id ~= "" and prefs[account_id]
    if type(preferred) == "number" and preferred >= 1 and preferred <= max_slot and preferred ~= slot then
      local occupant = next_occupied[preferred]
      if occupant and occupant ~= account_id then
        local without = {}
        for s, a in pairs(next_occupied) do
          if s ~= preferred then
            without[s] = a
          end
        end
        local new_slot = internal.choose_slot(without, prefs, occupant, max_slot)
        if new_slot then
          next_occupied[new_slot] = occupant
        end
      end
      next_occupied[preferred] = account_id
      next_occupied[slot] = nil
      moved = true
      return next_occupied, moved
    end
  end
  return next_occupied, moved
end

local function reconcile_settle(occupied, prefs, max_slot)
  for _ = 1, max_slot do
    local next_occupied, moved = reconcile_pass(occupied, prefs, max_slot)
    occupied = next_occupied
    if not moved then
      break
    end
  end
  return occupied
end

do
  local max_slot = 9
  local occupied = { [1] = "2", [2] = "1", [3] = "3" }
  local prefs = { ["1"] = 1, ["3"] = 2 }

  local after_first = reconcile_settle(occupied, prefs, max_slot)
  local _, moved_second = reconcile_pass(after_first, prefs, max_slot)
  check("reconcile_is_idempotent_second_pass_moves_nothing", moved_second, false)
  check("reconcile_places_account_1_on_its_slot", after_first[1], "1")
end

-- ── shell_single_quote: neutralizes everything a shell would otherwise treat
-- specially inside single quotes, given a correctly-escaped literal quote.

check("shq_plain", internal.shell_single_quote("Ana"), "'Ana'")
check("shq_single_quote", internal.shell_single_quote("O'Brien"), "'O'\\''Brien'")
check("shq_backtick", internal.shell_single_quote("`whoami`"), "'`whoami`'")
check("shq_dollar_paren", internal.shell_single_quote("$(rm -rf /)"), "'$(rm -rf /)'")
check("shq_newline", internal.shell_single_quote("line1\nline2"), "'line1\nline2'")
check("shq_semicolon", internal.shell_single_quote("a; rm -rf /"), "'a; rm -rf /'")

print("")
if failures == 0 then
  print("ALL OK")
  os.exit(0)
else
  print(failures .. " FAILURE(S)")
  os.exit(1)
end
