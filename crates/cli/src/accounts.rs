//! `accounts.json` (source of truth) and the generated `accounts.lua` read by
//! the Hyprland module.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum AccountsError {
    #[error("account {0} does not exist")]
    NotFound(String),
    #[error("unknown class '{input}'; valid classes: {valid}")]
    UnknownClass { input: String, valid: String },
    #[error("slot {slot} is already preferred by account {owner}")]
    SlotTaken { slot: u8, owner: String },
    #[error("slot must be between 1 and 9, got {0}")]
    SlotOutOfRange(u8),
    #[error("account {account} has no character {character}")]
    CharacterNotFound { account: String, character: String },
    #[error("account {0} has no currently active character; pass --character <hash>")]
    NoActiveCharacter(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slot: Option<u8>,
    /// Per-character overrides, keyed by the character's `Characters/<hash>`
    /// directory name. Reading an old `accounts.json` with no `characters`
    /// key parses this as an empty map; writing preserves it.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub characters: BTreeMap<String, Character>,
}

/// Where a character's `name`/`class` came from: set directly by the user
/// (`character set`), or inferred from a parsed window title (part C, never
/// overwrites a `user`-sourced value).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CharacterSource {
    #[default]
    User,
    Title,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Character {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class: Option<String>,
    /// The character's decimal id, filled in automatically once recovered
    /// from the character file and verified by MD5 against the directory
    /// hash. Never set by the user directly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub character_id: Option<String>,
    #[serde(default, skip_serializing_if = "is_default_source")]
    pub source: CharacterSource,
}

fn is_default_source(s: &CharacterSource) -> bool {
    *s == CharacterSource::User
}

/// `accounts.json`: a map from account id (the Dofus account id found in the
/// game client's log filename) to its configured display data.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Accounts(pub BTreeMap<String, Account>);

impl Accounts {
    pub fn load(path: &Path) -> Result<Self, AccountsError> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let json = fs::read_to_string(path)?;
        Ok(serde_json::from_str(&json)?)
    }

    /// Writes `accounts.json` atomically (write to `.tmp`, rename previous
    /// file to `.bak`, then rename into place).
    pub fn save(&self, path: &Path) -> Result<(), AccountsError> {
        let json = serde_json::to_string_pretty(&self.0)?;
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, json)?;
        if path.exists() {
            fs::rename(path, path.with_extension("json.bak"))?;
        }
        fs::rename(&tmp, path)?;
        Ok(())
    }

    /// Validates that `slot` is not already preferred by an account other
    /// than `account_id`.
    pub fn check_slot_free(&self, account_id: &str, slot: u8) -> Result<(), AccountsError> {
        if !(1..=9).contains(&slot) {
            return Err(AccountsError::SlotOutOfRange(slot));
        }
        if let Some((owner, _)) = self
            .0
            .iter()
            .find(|(id, acc)| id.as_str() != account_id && acc.slot == Some(slot))
        {
            return Err(AccountsError::SlotTaken {
                slot,
                owner: owner.clone(),
            });
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn set(
        &mut self,
        account_id: &str,
        name: Option<String>,
        class: Option<String>,
        slot: Option<u8>,
        clear_name: bool,
        clear_class: bool,
    ) -> Result<(), AccountsError> {
        if let Some(slot) = slot {
            self.check_slot_free(account_id, slot)?;
        }
        let entry = self.0.entry(account_id.to_string()).or_default();
        if clear_name {
            entry.name = None;
        } else if let Some(name) = name {
            entry.name = Some(name);
        }
        if clear_class {
            entry.class = None;
        } else if let Some(class) = class {
            entry.class = Some(class);
        }
        if let Some(slot) = slot {
            entry.slot = Some(slot);
        }
        Ok(())
    }

    pub fn remove(&mut self, account_id: &str) -> Result<(), AccountsError> {
        self.0
            .remove(account_id)
            .map(|_| ())
            .ok_or_else(|| AccountsError::NotFound(account_id.to_string()))
    }

    /// Creates or updates a character entry for `account_id`/`character_hash`.
    /// `source` defaults to `User` (a direct `character set` call); part C's
    /// title-parsing path passes `Title` and must not clobber a `User`
    /// value with a `Title` one (that ordering is enforced by the caller,
    /// not here, since only the caller knows which field actually changed).
    #[allow(clippy::too_many_arguments)]
    pub fn set_character(
        &mut self,
        account_id: &str,
        character_hash: &str,
        name: Option<String>,
        class: Option<String>,
        source: CharacterSource,
        clear_name: bool,
        clear_class: bool,
    ) -> Result<(), AccountsError> {
        let account = self.0.entry(account_id.to_string()).or_default();
        let character = account
            .characters
            .entry(character_hash.to_string())
            .or_default();
        if clear_name {
            character.name = None;
        } else if let Some(name) = name {
            character.name = Some(name);
        }
        if clear_class {
            character.class = None;
        } else if let Some(class) = class {
            character.class = Some(class);
        }
        // A clear puts the field back to unset so a later title-derived
        // value may fill it again; it does not by itself change `source`
        // (a character that still has its other field set by the user stays
        // `User`), but when clearing empties the character completely,
        // reset `source` too so the very next fill (title or user) starts
        // from a clean slate rather than inheriting a stale source.
        if (clear_name || clear_class) && character.name.is_none() && character.class.is_none() {
            character.source = CharacterSource::default();
        } else if !clear_name && !clear_class {
            character.source = source;
        }
        Ok(())
    }

    /// Applies a name/class parsed from a window title (Part C). Field-level:
    /// only fills a field that is still unset, and never overwrites a field
    /// that already has a value (whether it was set by the user or by an
    /// earlier title parse), so a value the user set by hand can never be
    /// clobbered by this path. `source` is only (re)marked `Title` when this
    /// call actually changed something; a character already fully labelled
    /// keeps its existing source untouched.
    pub fn apply_title_derived(
        &mut self,
        account_id: &str,
        character_hash: &str,
        name: Option<String>,
        class: Option<String>,
    ) -> Result<(), AccountsError> {
        let account = self.0.entry(account_id.to_string()).or_default();
        let character = account
            .characters
            .entry(character_hash.to_string())
            .or_default();

        let mut changed = false;
        if character.name.is_none() {
            if let Some(name) = name {
                character.name = Some(name);
                changed = true;
            }
        }
        if character.class.is_none() {
            if let Some(class) = class {
                character.class = Some(class);
                changed = true;
            }
        }
        if changed && character.source != CharacterSource::User {
            character.source = CharacterSource::Title;
        }
        Ok(())
    }

    pub fn remove_character(
        &mut self,
        account_id: &str,
        character_hash: &str,
    ) -> Result<(), AccountsError> {
        let account = self
            .0
            .get_mut(account_id)
            .ok_or_else(|| AccountsError::NotFound(account_id.to_string()))?;
        account
            .characters
            .remove(character_hash)
            .map(|_| ())
            .ok_or_else(|| AccountsError::CharacterNotFound {
                account: account_id.to_string(),
                character: character_hash.to_string(),
            })
    }
}

/// Resolves a class name (alias, any case, with or without accents) to its
/// canonical form, or lists the valid classes on failure.
pub fn resolve_class(input: &str) -> Result<String, AccountsError> {
    dofustabs_core::game_class::resolve_from_text(input)
        .map(|c| c.canonical_name.to_string())
        .ok_or_else(|| AccountsError::UnknownClass {
            input: input.to_string(),
            valid: dofustabs_core::game_class::ALL
                .iter()
                .map(|c| c.canonical_name)
                .collect::<Vec<_>>()
                .join(", "),
        })
}

/// Escapes a string for a Lua double-quoted literal.
pub fn lua_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            other => escaped.push(other),
        }
    }
    escaped
}

/// Renders `accounts.json` (plus the primary class, stored separately in
/// `config.json`) as the Lua table `accounts.lua` holds: keyed by account
/// id, each value `{ slot = n, name = "...", class = "...", characters =
/// { [hash] = { name = "...", class = "..." }, ... } }` with unset fields
/// omitted, plus a top-level `primary_class` field. Deterministic key order
/// (sorted, since `Accounts` is backed by a `BTreeMap`) so the generated
/// file diffs cleanly.
pub fn render_accounts_lua(accounts: &Accounts, primary_class: Option<&str>) -> String {
    let mut out = String::from("return {\n");
    if let Some(primary_class) = primary_class {
        out.push_str(&format!(
            "  primary_class = \"{}\",\n",
            lua_escape(primary_class)
        ));
    }
    out.push_str("  accounts = {\n");
    for (id, account) in &accounts.0 {
        let mut fields = Vec::new();
        if let Some(slot) = account.slot {
            fields.push(format!("slot = {slot}"));
        }
        if let Some(name) = &account.name {
            fields.push(format!("name = \"{}\"", lua_escape(name)));
        }
        if let Some(class) = &account.class {
            fields.push(format!("class = \"{}\"", lua_escape(class)));
        }
        if !account.characters.is_empty() {
            let mut char_entries = Vec::new();
            for (hash, character) in &account.characters {
                let mut cfields = Vec::new();
                if let Some(name) = &character.name {
                    cfields.push(format!("name = \"{}\"", lua_escape(name)));
                }
                if let Some(class) = &character.class {
                    cfields.push(format!("class = \"{}\"", lua_escape(class)));
                }
                char_entries.push(format!(
                    "      [\"{}\"] = {{ {} }},\n",
                    lua_escape(hash),
                    cfields.join(", ")
                ));
            }
            fields.push(format!("characters = {{\n{}    }}", char_entries.concat()));
        }
        out.push_str(&format!(
            "    [\"{}\"] = {{ {} }},\n",
            lua_escape(id),
            fields.join(", ")
        ));
    }
    out.push_str("  },\n}\n");
    out
}

/// Writes `accounts.lua` from `accounts`, atomically.
pub fn write_accounts_lua(
    path: &Path,
    accounts: &Accounts,
    primary_class: Option<&str>,
) -> Result<(), AccountsError> {
    let lua = render_accounts_lua(accounts, primary_class);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("lua.tmp");
    fs::write(&tmp, lua)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lua_escape_handles_backslash_quote_and_newline() {
        assert_eq!(lua_escape("plain"), "plain");
        assert_eq!(lua_escape(r#"say "hi""#), r#"say \"hi\""#);
        assert_eq!(lua_escape(r"C:\path"), r"C:\\path");
        assert_eq!(lua_escape("a\nb"), "a\\nb");
    }

    #[test]
    fn resolves_known_class_aliases() {
        assert_eq!(resolve_class("feca").unwrap(), "Feca");
        assert_eq!(resolve_class("Iop").unwrap(), "Yopuka");
        assert_eq!(resolve_class("Sacrieur").unwrap(), "Sacrógrito");
    }

    #[test]
    fn unknown_class_lists_valid_options() {
        let err = resolve_class("wizard").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("unknown class"));
        assert!(msg.contains("Feca"));
    }

    #[test]
    fn render_accounts_lua_omits_unset_fields_and_sorts_keys() {
        let mut accounts = Accounts::default();
        accounts
            .set("2", None, Some("Feca".to_string()), None, false, false)
            .unwrap();
        accounts
            .set("1", Some("Ana".to_string()), None, Some(3), false, false)
            .unwrap();

        let lua = render_accounts_lua(&accounts, None);
        assert_eq!(
            lua,
            "return {\n  accounts = {\n    [\"1\"] = { slot = 3, name = \"Ana\" },\n    [\"2\"] = { class = \"Feca\" },\n  },\n}\n"
        );
    }

    #[test]
    fn render_accounts_lua_escapes_names() {
        let mut accounts = Accounts::default();
        accounts
            .set(
                "1",
                Some("Ana \"Boss\"".to_string()),
                None,
                None,
                false,
                false,
            )
            .unwrap();
        let lua = render_accounts_lua(&accounts, None);
        assert!(lua.contains("name = \"Ana \\\"Boss\\\"\""));
    }

    #[test]
    fn render_accounts_lua_includes_primary_class_and_characters() {
        let mut accounts = Accounts::default();
        accounts
            .set("1", Some("Ana".to_string()), None, None, false, false)
            .unwrap();
        accounts
            .set_character(
                "1",
                "92d60605292e2c0cd060f461d152d096",
                Some("Anabel".to_string()),
                Some("Feca".to_string()),
                CharacterSource::User,
                false,
                false,
            )
            .unwrap();
        let lua = render_accounts_lua(&accounts, Some("Yopuka"));
        assert!(lua.starts_with("return {\n  primary_class = \"Yopuka\",\n"));
        assert!(lua.contains("characters = {\n"));
        assert!(lua.contains(
            "[\"92d60605292e2c0cd060f461d152d096\"] = { name = \"Anabel\", class = \"Feca\" }"
        ));
    }

    #[test]
    fn set_character_creates_and_updates_entries() {
        let mut accounts = Accounts::default();
        accounts
            .set_character(
                "1",
                "hash1",
                Some("Ana".to_string()),
                Some("Feca".to_string()),
                CharacterSource::User,
                false,
                false,
            )
            .unwrap();
        assert_eq!(
            accounts.0["1"].characters["hash1"].name,
            Some("Ana".to_string())
        );
        accounts
            .set_character(
                "1",
                "hash1",
                None,
                Some("Yopuka".to_string()),
                CharacterSource::Title,
                false,
                false,
            )
            .unwrap();
        assert_eq!(
            accounts.0["1"].characters["hash1"].class,
            Some("Yopuka".to_string())
        );
        assert_eq!(
            accounts.0["1"].characters["hash1"].name,
            Some("Ana".to_string())
        );
        assert_eq!(
            accounts.0["1"].characters["hash1"].source,
            CharacterSource::Title
        );
    }

    #[test]
    fn remove_character_removes_only_that_character() {
        let mut accounts = Accounts::default();
        accounts
            .set_character(
                "1",
                "hash1",
                None,
                None,
                CharacterSource::User,
                false,
                false,
            )
            .unwrap();
        accounts
            .set_character(
                "1",
                "hash2",
                None,
                None,
                CharacterSource::User,
                false,
                false,
            )
            .unwrap();
        accounts.remove_character("1", "hash1").unwrap();
        assert!(!accounts.0["1"].characters.contains_key("hash1"));
        assert!(accounts.0["1"].characters.contains_key("hash2"));
    }

    #[test]
    fn clear_class_and_name_reset_the_field_and_the_source() {
        let mut accounts = Accounts::default();
        accounts
            .set_character(
                "1",
                "hash1",
                Some("Ana".to_string()),
                Some("Feca".to_string()),
                CharacterSource::User,
                false,
                false,
            )
            .unwrap();
        accounts
            .set_character("1", "hash1", None, None, CharacterSource::User, false, true)
            .unwrap();
        assert_eq!(accounts.0["1"].characters["hash1"].class, None);
        assert_eq!(
            accounts.0["1"].characters["hash1"].name,
            Some("Ana".to_string())
        );
        accounts
            .set_character("1", "hash1", None, None, CharacterSource::User, true, false)
            .unwrap();
        assert_eq!(accounts.0["1"].characters["hash1"].name, None);
        assert_eq!(
            accounts.0["1"].characters["hash1"].source,
            CharacterSource::User
        );
    }

    #[test]
    fn clear_account_class_falls_back_to_empty() {
        let mut accounts = Accounts::default();
        accounts
            .set("1", None, Some("Feca".to_string()), None, false, false)
            .unwrap();
        accounts.set("1", None, None, None, false, true).unwrap();
        assert_eq!(accounts.0["1"].class, None);
    }

    #[test]
    fn remove_character_missing_is_an_error() {
        let mut accounts = Accounts::default();
        accounts.set("1", None, None, None, false, false).unwrap();
        assert!(matches!(
            accounts.remove_character("1", "nope"),
            Err(AccountsError::CharacterNotFound { .. })
        ));
    }

    #[test]
    fn remove_character_unknown_account_is_an_error() {
        let mut accounts = Accounts::default();
        assert!(matches!(
            accounts.remove_character("999", "hash1"),
            Err(AccountsError::NotFound(_))
        ));
    }

    #[test]
    fn old_accounts_json_without_characters_key_loads_fine() {
        let json = r#"{"1": {"name": "Ana", "class": "Feca", "slot": 1}}"#;
        let accounts: Accounts = serde_json::from_str(json).unwrap();
        assert!(accounts.0["1"].characters.is_empty());
    }

    #[test]
    fn slot_conflict_is_rejected() {
        let mut accounts = Accounts::default();
        accounts
            .set("1", None, None, Some(5), false, false)
            .unwrap();
        let err = accounts
            .set("2", None, None, Some(5), false, false)
            .unwrap_err();
        assert!(matches!(err, AccountsError::SlotTaken { slot: 5, .. }));
    }

    #[test]
    fn slot_can_be_reassigned_to_the_same_account() {
        let mut accounts = Accounts::default();
        accounts
            .set("1", None, None, Some(5), false, false)
            .unwrap();
        accounts
            .set("1", None, None, Some(5), false, false)
            .unwrap();
        assert_eq!(accounts.0["1"].slot, Some(5));
    }

    #[test]
    fn slot_out_of_range_is_rejected() {
        let mut accounts = Accounts::default();
        assert!(matches!(
            accounts.set("1", None, None, Some(0), false, false),
            Err(AccountsError::SlotOutOfRange(0))
        ));
        assert!(matches!(
            accounts.set("1", None, None, Some(10), false, false),
            Err(AccountsError::SlotOutOfRange(10))
        ));
    }

    #[test]
    fn remove_missing_account_is_an_error() {
        let mut accounts = Accounts::default();
        assert!(matches!(
            accounts.remove("999"),
            Err(AccountsError::NotFound(_))
        ));
    }

    #[test]
    fn save_and_load_roundtrip_with_backup() {
        let dir = std::env::temp_dir().join(format!("dofustabs-cli-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("accounts.json");

        let mut accounts = Accounts::default();
        accounts
            .set(
                "1",
                Some("Ana".to_string()),
                Some("Feca".to_string()),
                Some(1),
                false,
                false,
            )
            .unwrap();
        accounts.save(&path).unwrap();
        assert_eq!(Accounts::load(&path).unwrap(), accounts);

        accounts
            .set("1", Some("Beto".to_string()), None, None, false, false)
            .unwrap();
        accounts.save(&path).unwrap();
        assert!(path.with_extension("json.bak").exists());

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn missing_accounts_file_loads_as_empty() {
        let path = Path::new("/nonexistent/dofustabs-cli-test/accounts.json");
        assert_eq!(Accounts::load(path).unwrap(), Accounts::default());
    }
}
