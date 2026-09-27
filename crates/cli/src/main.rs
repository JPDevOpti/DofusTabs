mod accounts;
mod assets;
mod config;
mod doctor;
mod identify;
mod install;
mod launch;
mod mouse;
mod paths;
mod quit;
mod state;

use clap::{Parser, Subcommand};

use accounts::{resolve_class, write_accounts_lua, Accounts, CharacterSource};

#[derive(Parser)]
#[command(
    name = "dofustabs",
    about = "Isolated Dofus game workspaces for Hyprland"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Manage account metadata (name, class, preferred slot).
    Accounts {
        #[command(subcommand)]
        action: AccountsAction,
    },
    /// Show currently running clients, from state.json.
    Status,
    /// Check the integration end-to-end.
    Doctor,
    /// Install the module, widget, icons and config skeleton.
    Install,
    /// Remove everything `install` set up (keeps accounts.json).
    Uninstall,
    /// Start the Ankama launcher on the dedicated GPU.
    Launch,
    /// Manage per-character metadata (name, class).
    Character {
        #[command(subcommand)]
        action: CharacterAction,
    },
    /// Manage the primary-class slot override.
    Primary {
        #[command(subcommand)]
        action: PrimaryAction,
    },
    /// Resolve the active character of every running client.
    Identify {
        /// Print the resolution without writing any file or calling
        /// hyprctl.
        #[arg(long)]
        dry_run: bool,
    },
    /// Close every running game client and the launcher.
    Quit {
        /// Leave the Ankama launcher running; only close game clients.
        #[arg(long)]
        keep_launcher: bool,
        /// Print what would be closed and signal nothing.
        #[arg(long)]
        dry_run: bool,
    },
    /// MX Master 3S Smart Shift button target: focus slot 1 (or the lowest
    /// occupied slot) if a client is running, otherwise toggle the wheel's
    /// ratchet mode. Called by the Solaar rule installed by `mouse enable`.
    Main,
    /// Manage the MX Master 3S Smart Shift integration (Solaar diversion,
    /// rule, autostart).
    Mouse {
        #[command(subcommand)]
        action: MouseAction,
    },
}

#[derive(Subcommand)]
enum MouseAction {
    /// Divert Smart Shift, install the Solaar rule and autostart entry, and
    /// start Solaar now if it is not running.
    Enable,
    /// Reverse everything `enable` did.
    Disable,
    /// Report Solaar/diversion/rule/autostart/bind state.
    Status,
}

#[derive(Subcommand)]
enum AccountsAction {
    /// List every known account.
    List,
    /// Create or update an account's name, class and/or preferred slot.
    Set {
        account_id: String,
        #[arg(long, conflicts_with = "clear_name")]
        name: Option<String>,
        #[arg(long, conflicts_with = "clear_class")]
        class: Option<String>,
        #[arg(long, value_parser = clap::value_parser!(u8).range(1..=9))]
        slot: Option<u8>,
        /// Clears the name (falls back to "Account <id>").
        #[arg(long)]
        clear_name: bool,
        /// Clears the class.
        #[arg(long)]
        clear_class: bool,
    },
    /// Remove an account.
    Remove { account_id: String },
}

#[derive(Subcommand)]
enum CharacterAction {
    /// Create or update a character's name/class.
    Set {
        #[arg(long)]
        account: String,
        /// Character hash (`Characters/<hash>`). Defaults to the account's
        /// currently active character, from the last `dofustabs identify`.
        #[arg(long)]
        character: Option<String>,
        #[arg(long, conflicts_with = "clear_name")]
        name: Option<String>,
        #[arg(long, conflicts_with = "clear_class")]
        class: Option<String>,
        /// Clears the name (falls back to the account's, then to
        /// "Account <id>"). Resets the label's source too, so a later
        /// title-derived value may fill it again.
        #[arg(long)]
        clear_name: bool,
        /// Clears the class.
        #[arg(long)]
        clear_class: bool,
    },
    /// List every configured character, grouped by account.
    List,
    /// Remove a character entry.
    Remove {
        #[arg(long)]
        account: String,
        #[arg(long)]
        character: String,
    },
    /// Parses a window title and stores the resulting name/class for the
    /// account's active character, without overwriting a field a user set
    /// directly. Part C: not exercisable live on Linux today (the client
    /// never puts the character name/class in the title there), but the
    /// Hyprland module still wires this up for the day it does (or for a
    /// platform/build where it does).
    FromTitle {
        #[arg(long)]
        account: String,
        #[arg(long)]
        title: String,
    },
}

#[derive(Subcommand)]
enum PrimaryAction {
    /// Set the primary class.
    Set { class: String },
    /// Clear the primary class.
    Clear,
    /// Show the configured primary class, if any.
    Show,
}

fn main() -> anyhow::Result<()> {
    env_logger::init();
    let cli = Cli::parse();

    match cli.command {
        Commands::Accounts { action } => run_accounts(action),
        Commands::Status => run_status(),
        Commands::Doctor => {
            if doctor::run() {
                Ok(())
            } else {
                std::process::exit(1);
            }
        }
        Commands::Install => install::install(),
        Commands::Uninstall => install::uninstall(),
        Commands::Launch => launch::run(),
        Commands::Character { action } => run_character(action),
        Commands::Primary { action } => run_primary(action),
        Commands::Identify { dry_run } => identify::run(dry_run),
        Commands::Quit {
            keep_launcher,
            dry_run,
        } => quit::run(keep_launcher, dry_run),
        Commands::Main => mouse::run_main(),
        Commands::Mouse { action } => match action {
            MouseAction::Enable => mouse::run_enable(),
            MouseAction::Disable => mouse::run_disable(),
            MouseAction::Status => mouse::run_status(),
        },
    }
}

fn run_accounts(action: AccountsAction) -> anyhow::Result<()> {
    let json_path = paths::accounts_json_path();

    match action {
        AccountsAction::List => {
            let accounts = Accounts::load(&json_path)?;
            if accounts.0.is_empty() {
                println!("no accounts configured yet");
                return Ok(());
            }
            println!("{:<14} {:<6} {:<16} NAME", "ACCOUNT", "SLOT", "CLASS");
            for (id, account) in &accounts.0 {
                println!(
                    "{:<14} {:<6} {:<16} {}",
                    id,
                    account.slot.map(|s| s.to_string()).unwrap_or_default(),
                    account.class.clone().unwrap_or_default(),
                    account.name.clone().unwrap_or_default(),
                );
            }
            Ok(())
        }
        AccountsAction::Set {
            account_id,
            name,
            class,
            slot,
            clear_name,
            clear_class,
        } => {
            let class = class.map(|c| resolve_class(&c)).transpose()?;
            let mut accounts = Accounts::load(&json_path)?;
            accounts.set(&account_id, name, class, slot, clear_name, clear_class)?;
            accounts.save(&json_path)?;
            write_generated_lua(&accounts)?;
            println!("updated account {account_id}");
            notify_hyprland();
            Ok(())
        }
        AccountsAction::Remove { account_id } => {
            let mut accounts = Accounts::load(&json_path)?;
            accounts.remove(&account_id)?;
            accounts.save(&json_path)?;
            write_generated_lua(&accounts)?;
            println!("removed account {account_id}");
            notify_hyprland();
            Ok(())
        }
    }
}

/// Writes `accounts.lua` from `accounts` plus the currently configured
/// primary class, at the standard path.
fn write_generated_lua(accounts: &Accounts) -> anyhow::Result<()> {
    let primary_class = config::read_primary_class(&paths::cli_config_json_path())?;
    write_accounts_lua(
        &paths::accounts_lua_path(),
        accounts,
        primary_class.as_deref(),
    )?;
    Ok(())
}

fn run_character(action: CharacterAction) -> anyhow::Result<()> {
    let json_path = paths::accounts_json_path();
    match action {
        CharacterAction::Set {
            account,
            character,
            name,
            class,
            clear_name,
            clear_class,
        } => {
            let class = class.map(|c| resolve_class(&c)).transpose()?;
            let character_hash = match character {
                Some(h) => h,
                None => resolve_active_character_hash(&account)?,
            };
            let mut accounts = Accounts::load(&json_path)?;
            accounts.set_character(
                &account,
                &character_hash,
                name,
                class,
                CharacterSource::User,
                clear_name,
                clear_class,
            )?;
            accounts.save(&json_path)?;
            write_generated_lua(&accounts)?;
            println!("updated character {character_hash} for account {account}");
            notify_hyprland();
            Ok(())
        }
        CharacterAction::List => {
            let accounts = Accounts::load(&json_path)?;
            if accounts.0.iter().all(|(_, a)| a.characters.is_empty()) {
                println!("no characters configured yet");
                return Ok(());
            }
            for (account_id, account) in &accounts.0 {
                for (hash, character) in &account.characters {
                    println!(
                        "{:<14} {:<32} {:<16} {:<10} {}",
                        account_id,
                        hash,
                        character.name.clone().unwrap_or_default(),
                        character.class.clone().unwrap_or_default(),
                        match character.source {
                            CharacterSource::User => "user",
                            CharacterSource::Title => "title",
                        },
                    );
                }
            }
            Ok(())
        }
        CharacterAction::Remove { account, character } => {
            let mut accounts = Accounts::load(&json_path)?;
            accounts.remove_character(&account, &character)?;
            accounts.save(&json_path)?;
            write_generated_lua(&accounts)?;
            println!("removed character {character} for account {account}");
            notify_hyprland();
            Ok(())
        }
        CharacterAction::FromTitle { account, title } => {
            if title.trim() == "Dofus" {
                return Ok(()); // bare game title carries no character info
            }
            let parsed = dofustabs_core::window_title::parse_title(&title);
            let class = parsed.class.map(|c| c.canonical_name.to_string());
            if class.is_none() && parsed.character_name.trim().is_empty() {
                return Ok(()); // nothing usable in this title
            }
            let character_hash = resolve_active_character_hash(&account)?;
            let name = if parsed.character_name.trim().is_empty() {
                None
            } else {
                Some(parsed.character_name)
            };
            let mut accounts = Accounts::load(&json_path)?;
            accounts.apply_title_derived(&account, &character_hash, name, class)?;
            accounts.save(&json_path)?;
            write_generated_lua(&accounts)?;
            notify_hyprland();
            Ok(())
        }
    }
}

/// Looks up `account`'s currently active character from the last
/// `dofustabs identify` run, for `character set` calls that omit
/// `--character`.
fn resolve_active_character_hash(account: &str) -> anyhow::Result<String> {
    let identity = identify::Identity::load(&paths::identity_json_path())?;
    identity
        .active_character(account)
        .map(str::to_string)
        .ok_or_else(|| accounts::AccountsError::NoActiveCharacter(account.to_string()).into())
}

fn run_primary(action: PrimaryAction) -> anyhow::Result<()> {
    let config_path = paths::cli_config_json_path();
    match action {
        PrimaryAction::Set { class } => {
            let class = resolve_class(&class)?;
            config::write_primary_class(&config_path, Some(&class))?;
            let accounts = Accounts::load(&paths::accounts_json_path())?;
            write_generated_lua(&accounts)?;
            println!("primary class set to {class}");
            notify_hyprland();
            Ok(())
        }
        PrimaryAction::Clear => {
            config::write_primary_class(&config_path, None)?;
            let accounts = Accounts::load(&paths::accounts_json_path())?;
            write_generated_lua(&accounts)?;
            println!("primary class cleared");
            notify_hyprland();
            Ok(())
        }
        PrimaryAction::Show => {
            match config::read_primary_class(&config_path)? {
                Some(c) => println!("{c}"),
                None => println!("no primary class configured"),
            }
            Ok(())
        }
    }
}

/// Best-effort `hyprctl eval 'dofustabs.refresh()'` so the bar updates
/// immediately; failure is expected when Hyprland is not running and is only
/// hinted at, never propagated.
fn notify_hyprland() {
    let status = std::process::Command::new("hyprctl")
        .args(["eval", "dofustabs.refresh()"])
        .status();
    match status {
        Ok(s) if s.success() => {}
        _ => eprintln!(
            "hint: could not refresh the running module (is Hyprland running with dofustabs installed?); \
             the change is saved and will apply on the next window event or reload"
        ),
    }
}

fn run_status() -> anyhow::Result<()> {
    let accounts = Accounts::load(&paths::accounts_json_path()).unwrap_or_default();
    let identity = identify::Identity::load(&paths::identity_json_path()).unwrap_or_default();
    match state::read(&paths::state_json_path()) {
        Ok(s) if s.clients.is_empty() => {
            println!("no clients running");
            Ok(())
        }
        Ok(s) => {
            println!(
                "{:<5} {:<11} {:<14} {:<16} {:<12} {:<10} PID",
                "SLOT", "WORKSPACE", "ACCOUNT", "NAME", "CLASS", "CHARACTER"
            );
            for c in &s.clients {
                let active_hash = identity.active_character(&c.account);
                let short_hash = active_hash.map(|h| &h[..h.len().min(8)]).unwrap_or("-");
                println!(
                    "{:<5} {:<11} {:<14} {:<16} {:<12} {:<10} {}",
                    c.slot, c.workspace, c.account, c.name, c.class, short_hash, c.pid
                );
                let account = accounts.0.get(&c.account);
                let has_character_entry = active_hash
                    .and_then(|h| account.map(|a| a.characters.contains_key(h)))
                    .unwrap_or(false);
                if !has_character_entry {
                    match active_hash {
                        Some(_) => println!(
                            "  -> ACTIVE character not labelled yet: dofustabs character set --account {} --name <name> --class <class>",
                            c.account
                        ),
                        None => println!(
                            "  -> not in game yet (or run `dofustabs identify`); account-level fallback: dofustabs accounts set {} --name <name> --class <class>",
                            c.account
                        ),
                    }
                }
            }
            Ok(())
        }
        Err(state::StateError::NotFound(path)) => {
            println!(
                "state file not found at {path}; is the module installed and a client running?"
            );
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}
