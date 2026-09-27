//! `dofustabs quit`: closes every running game client and, unless
//! `--keep-launcher` is given, the Ankama launcher too.

use std::thread::sleep;
use std::time::Duration;

use dofustabs_core::quit::{escalate_to_kill, escalate_to_term, summarize};
use dofustabs_platform::linux::quit as backend;

const WINDOW_CLOSE_WAIT: Duration = Duration::from_secs(3);
const TERM_WAIT: Duration = Duration::from_secs(5);

pub fn run(keep_launcher: bool, dry_run: bool) -> anyhow::Result<()> {
    let clients = backend::discover_game_clients()?;
    let uid = backend::current_uid();
    let launcher = if keep_launcher {
        backend::LauncherProcesses::default()
    } else {
        backend::discover_launcher_processes(uid)
    };

    if dry_run {
        print_dry_run(&clients, &launcher);
        return Ok(());
    }

    let mut anything_running = !clients.is_empty();
    let game_summary = close_game_clients(&clients)?;
    println!(
        "closed {} game client(s) ({} needed SIGTERM, {} needed SIGKILL)",
        clients.len(),
        game_summary.needed_term,
        game_summary.needed_kill
    );

    if !keep_launcher {
        let launcher_pids = launcher.all_pids();
        anything_running = anything_running || !launcher_pids.is_empty();
        if !launcher_pids.is_empty() {
            close_launcher(&launcher)?;
            println!("closed launcher ({} process(es))", launcher_pids.len());
        }
    }

    if !anything_running {
        println!("nothing was running");
    }

    let anything_left = clients.iter().any(|c| backend::pid_alive(c.pid))
        || launcher.all_pids().iter().any(|&p| backend::pid_alive(p));
    if anything_left {
        anyhow::bail!("some processes could not be closed");
    }
    Ok(())
}

fn print_dry_run(clients: &[backend::GameClient], launcher: &backend::LauncherProcesses) {
    println!("would close {} game client(s):", clients.len());
    for c in clients {
        println!("  pid {} (window {})", c.pid, c.address);
    }
    let pids = launcher.all_pids();
    println!("would close {} launcher process(es):", pids.len());
    for pid in &pids {
        let role = if launcher.main_zaap == Some(*pid) {
            "main zaap"
        } else if launcher.appimage.contains(pid) {
            "Dofus.AppImage"
        } else {
            "zaap (helper)"
        };
        println!("  pid {pid} ({role})");
    }
}

fn close_game_clients(
    clients: &[backend::GameClient],
) -> anyhow::Result<dofustabs_core::quit::EscalationSummary> {
    let all_pids: Vec<u32> = clients.iter().map(|c| c.pid).collect();
    if all_pids.is_empty() {
        return Ok(dofustabs_core::quit::EscalationSummary::default());
    }

    for c in clients {
        // Best-effort: a window that already closed is not an error.
        let _ = backend::request_close_window(&c.address);
    }
    sleep(WINDOW_CLOSE_WAIT);
    let alive_after_close: Vec<u32> = all_pids
        .iter()
        .copied()
        .filter(|&p| backend::pid_alive(p))
        .collect();

    for (pid, _) in escalate_to_term(&alive_after_close) {
        backend::send_signal(pid, "-TERM")?;
    }
    if !alive_after_close.is_empty() {
        sleep(TERM_WAIT);
    }
    let alive_after_term: Vec<u32> = alive_after_close
        .iter()
        .copied()
        .filter(|&p| backend::pid_alive(p))
        .collect();

    for (pid, _) in escalate_to_kill(&alive_after_term) {
        backend::send_signal(pid, "-KILL")?;
    }

    Ok(summarize(&all_pids, &alive_after_close, &alive_after_term))
}

fn close_launcher(launcher: &backend::LauncherProcesses) -> anyhow::Result<()> {
    if let Some(main) = launcher.main_zaap {
        backend::send_signal(main, "-TERM")?;
        sleep(TERM_WAIT);
    }
    // SIGKILL whatever is left: the main zaap (if SIGTERM did not work),
    // every helper zaap process, and the AppImage process.
    for pid in launcher.all_pids() {
        if backend::pid_alive(pid) {
            backend::send_signal(pid, "-KILL")?;
        }
    }
    Ok(())
}
