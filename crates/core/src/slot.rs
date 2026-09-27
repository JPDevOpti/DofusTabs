//! Pure decision logic for the primary-class slot override.
//!
//! This is a reference implementation: the Hyprland module's `choose_slot`
//! (Lua) implements the actual reserved-slot/reconcile algorithm described in
//! docs/WORKSPACES.md; this function computes the one input that algorithm
//! needs once the primary-class rule is added — each running account's
//! *effective* preferred slot, after the primary-class override is applied
//! on top of the account-level preferred slot from `accounts.json`. Porting
//! this rule into `dofustabs.lua` is stage 2's job; this module exists so
//! the rule itself is specified and tested once, in one place, rather than
//! only inside Lua.
//!
//! Rule (see docs/WORKSPACES.md "Slot rules"): the running client whose
//! *active* character has the configured primary class gets an effective
//! preferred slot of 1, overriding any other account's preference for slot
//! 1 (their preference for every other slot is unaffected). If two running
//! clients both have the primary class, the lowest account id gets slot 1
//! and the other is treated as having no preference at all (not even its
//! own account-level preference), so the rule never yields two winners.

use std::collections::BTreeMap;

/// One running client, as far as the slot-override rule cares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunningClient {
    pub account_id: String,
    /// The class of this client's currently active character, falling back
    /// to the account-level class when no character is resolved yet; `None`
    /// when neither is known.
    pub effective_class: Option<String>,
}

/// Computes the effective preferred slot for every account in
/// `account_slot_prefs` that is currently running, applying the
/// primary-class override on top.
///
/// - Accounts not in `running` keep no effective slot here (they are not
///   competing for a slot right now).
/// - `account_slot_prefs` holds each account's own preferred slot as read
///   from `accounts.json` (`slot`), if any.
/// - `primary_class` is the canonical class name from `primary set`, if
///   configured.
pub fn effective_preferred_slots(
    running: &[RunningClient],
    account_slot_prefs: &BTreeMap<String, u8>,
    primary_class: Option<&str>,
) -> BTreeMap<String, Option<u8>> {
    let mut result: BTreeMap<String, Option<u8>> = BTreeMap::new();
    for client in running {
        let own_pref = account_slot_prefs.get(&client.account_id).copied();
        result.insert(client.account_id.clone(), own_pref);
    }

    let Some(primary_class) = primary_class else {
        return result;
    };

    // Every running account whose active/account class matches the primary
    // class, in ascending account-id order (so the winner pick below is
    // deterministic: lowest account id first).
    let mut primary_holders: Vec<&str> = running
        .iter()
        .filter(|c| {
            c.effective_class
                .as_deref()
                .is_some_and(|class| class.eq_ignore_ascii_case(primary_class))
        })
        .map(|c| c.account_id.as_str())
        .collect();
    primary_holders.sort_unstable();
    primary_holders.dedup();

    let Some((winner, losers)) = primary_holders.split_first() else {
        return result;
    };

    // The winner takes slot 1 unconditionally.
    result.insert(winner.to_string(), Some(1));

    // Every other running account that preferred slot 1 loses that
    // preference (it is overridden, not merely displaced), since slot 1 is
    // now reserved for the primary-class holder.
    for (account_id, slot) in result.iter_mut() {
        if account_id != winner && *slot == Some(1) {
            *slot = None;
        }
    }

    // Any additional primary-class holder beyond the winner is treated as
    // having no preference at all, per the rule.
    for loser in losers {
        result.insert(loser.to_string(), None);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client(id: &str, class: Option<&str>) -> RunningClient {
        RunningClient {
            account_id: id.to_string(),
            effective_class: class.map(str::to_string),
        }
    }

    #[test]
    fn no_primary_class_configured_keeps_account_prefs_untouched() {
        let running = vec![client("1", Some("Feca")), client("2", Some("Yopuka"))];
        let mut prefs = BTreeMap::new();
        prefs.insert("2".to_string(), 1u8);
        let result = effective_preferred_slots(&running, &prefs, None);
        assert_eq!(result.get("1"), Some(&None));
        assert_eq!(result.get("2"), Some(&Some(1)));
    }

    #[test]
    fn primary_class_holder_gets_slot_1_overriding_others_preference() {
        let running = vec![client("1", Some("Yopuka")), client("2", Some("Feca"))];
        let mut prefs = BTreeMap::new();
        prefs.insert("2".to_string(), 1u8);
        prefs.insert("1".to_string(), 3u8);
        let result = effective_preferred_slots(&running, &prefs, Some("Yopuka"));
        assert_eq!(result.get("1"), Some(&Some(1)));
        // account 2's own slot-1 preference is overridden, not merely displaced.
        assert_eq!(result.get("2"), Some(&None));
    }

    #[test]
    fn other_preferences_are_unaffected() {
        let running = vec![client("1", Some("Yopuka")), client("2", Some("Feca"))];
        let mut prefs = BTreeMap::new();
        prefs.insert("2".to_string(), 4u8);
        let result = effective_preferred_slots(&running, &prefs, Some("Yopuka"));
        assert_eq!(result.get("2"), Some(&Some(4)));
    }

    #[test]
    fn two_primary_class_holders_lowest_account_id_wins() {
        let running = vec![client("200", Some("Yopuka")), client("100", Some("Yopuka"))];
        let result = effective_preferred_slots(&running, &BTreeMap::new(), Some("Yopuka"));
        assert_eq!(result.get("100"), Some(&Some(1)));
        assert_eq!(result.get("200"), Some(&None));
    }

    #[test]
    fn class_match_is_case_insensitive() {
        let running = vec![client("1", Some("yopuka"))];
        let result = effective_preferred_slots(&running, &BTreeMap::new(), Some("Yopuka"));
        assert_eq!(result.get("1"), Some(&Some(1)));
    }

    #[test]
    fn no_running_primary_class_holder_leaves_prefs_untouched() {
        let running = vec![client("1", Some("Feca"))];
        let mut prefs = BTreeMap::new();
        prefs.insert("1".to_string(), 1u8);
        let result = effective_preferred_slots(&running, &prefs, Some("Yopuka"));
        assert_eq!(result.get("1"), Some(&Some(1)));
    }

    #[test]
    fn unknown_class_is_not_a_primary_holder() {
        let running = vec![client("1", None)];
        let result = effective_preferred_slots(&running, &BTreeMap::new(), Some("Yopuka"));
        assert_eq!(result.get("1"), Some(&None));
    }
}
