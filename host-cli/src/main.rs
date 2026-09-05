//! myboot (host CLI) — the OS-side management tool (report §3.10.3). A normal std
//! binary run from Linux to install MyBoot, edit its config, inspect firmware
//! boot state, and — crucially — "bless" a boot as good (the OS side of the
//! transactional confirm/rollback protocol, mirroring `systemd-bless-boot`).
//!
//! It talks to firmware through efivarfs (`/sys/firmware/efi/efivars`), which is
//! the standard Linux interface `efibootmgr` also uses.

mod commands;
mod efivars;

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map(String::as_str).unwrap_or("help");
    let rest = &args[args.len().min(2)..];

    let result = match cmd {
        "install" => commands::install::run(rest),
        "uninstall" => commands::uninstall::run(rest),
        "status" => commands::status::run(),
        "bless" => commands::bless::run(rest),
        "config" => commands::config::run(rest),
        "boot-order" => commands::boot_order::run(),
        "help" | "-h" | "--help" => { commands::print_help(); Ok(()) }
        other => { eprintln!("unknown command: {other}\n"); commands::print_help(); Err("unknown command".into()) }
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => { eprintln!("error: {e}"); ExitCode::FAILURE }
    }
}
