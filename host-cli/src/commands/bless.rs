//! `myboot bless` — the OS side of the transactional confirm protocol
//! (report §3.7). After the system has booted and is deemed healthy, this marks
//! the pending boot as confirmed so MyBoot commits it (and does not roll back on
//! the next boot). Mirrors `systemd-bless-boot good`.
//!
//! It sets MyBoot's vendor `BootAttempt` variable to a "confirmed" marker for the
//! given entry (or, with no --entry, flips the existing pending marker's flag).
use crate::efivars::{self, VENDOR_GUID};
use super::CmdResult;

const VAR: &str = "BootAttempt";

pub fn run(args: &[String]) -> CmdResult {
    if !efivars::is_available() {
        return Err("efivarfs not available (need an EFI system and root)".into());
    }

    // Optional: --entry <id>
    let entry = parse_entry(args);

    let marker = match entry {
        Some(id) => format!("1\t{id}"),
        None => {
            // flip the existing pending marker to confirmed
            match efivars::read(VAR, VENDOR_GUID)? {
                Some(bytes) => {
                    let s = String::from_utf8(bytes).map_err(|_| "corrupt BootAttempt marker")?;
                    let id = s.splitn(2, '\t').nth(1).unwrap_or("").to_string();
                    if id.is_empty() { return Err("no pending boot to bless".into()); }
                    format!("1\t{id}")
                }
                None => return Err("no pending boot marker found".into()),
            }
        }
    };

    efivars::write(VAR, VENDOR_GUID, marker.as_bytes())?;
    println!("blessed: {}", marker.replace('\t', " "));
    Ok(())
}

fn parse_entry(args: &[String]) -> Option<String> {
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == "--entry" { return it.next().cloned(); }
    }
    None
}
