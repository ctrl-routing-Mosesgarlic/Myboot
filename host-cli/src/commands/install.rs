//! `myboot install` — place MyBoot on the ESP and create its config
//! (report §3.10.3). This is what CREATES `<ESP>/EFI/MyBoot/config.toml`
//! (and copies the built `myboot.efi`), answering "where does the config come
//! from". It is deliberately conservative: it never overwrites an existing
//! config, and it prints the exact `efibootmgr` command to register the entry
//! rather than writing NVRAM itself (so the user stays in control).
use super::CmdResult;
use std::fs;
use std::path::{Path, PathBuf};

/// Candidate ESP mount points, in order. `/boot/efi` is common on Debian/Fedora;
/// `/boot` is used by systemd-boot setups and NixOS; `/efi` by some others.
const ESP_CANDIDATES: &[&str] = &["/boot/efi", "/boot", "/efi"];
/// Where MyBoot installs itself on the ESP.
const INSTALL_SUBDIR: &str = "EFI/MyBoot";

const DEFAULT_CONFIG: &str = "\
# MyBoot configuration (created by `myboot install`).
# Edit `default` to the entry id you want to boot by default, then run
# `myboot config validate` to check it.

default = \"nixos:gen-current\"
timeout_secs = 5
policy = \"last-good-then-default\"   # or \"default\"
confirm = true
max_tries = 3
rollback = \"last-good\"               # or \"fallback\" / \"none\"

# Optional explicit entries (discovery finds most automatically):
# [[entry]]
# id = \"windows:default\"
# method = \"efi-chainload\"
# loader = \"\\\\EFI\\\\Microsoft\\\\Boot\\\\bootmgfw.efi\"
";

pub fn run(args: &[String]) -> CmdResult {
    let efi = flag_value(args, "--efi").map(PathBuf::from);
    let esp = match flag_value(args, "--esp") {
        Some(p) => {
            let path = PathBuf::from(&p);
            if !path.join("EFI").is_dir() && !path.is_dir() {
                return Err(format!("ESP not found at {p}").into());
            }
            path
        }
        None => detect_esp().ok_or(
            "could not autodetect the ESP; pass --esp <mountpoint> (e.g. /boot or /boot/efi)")?,
    };
    println!("using ESP: {}", esp.display());

    // 1. Create EFI/MyBoot/ and write a default config if absent.
    let install_dir = esp.join(INSTALL_SUBDIR);
    fs::create_dir_all(&install_dir).map_err(|e| format!("create {}: {e}", install_dir.display()))?;
    let cfg = install_dir.join("config.toml");
    if cfg.exists() {
        println!("config exists, left unchanged: {}", cfg.display());
    } else {
        fs::write(&cfg, DEFAULT_CONFIG).map_err(|e| format!("write {}: {e}", cfg.display()))?;
        println!("created config: {}", cfg.display());
    }

    // 2. Copy the EFI binary if provided (into EFI/MyBoot and the fallback path).
    if let Some(src) = efi {
        if !src.is_file() {
            return Err(format!("EFI binary not found: {}", src.display()).into());
        }
        let dst = install_dir.join("BOOTX64.EFI");
        fs::copy(&src, &dst).map_err(|e| format!("copy efi: {e}"))?;
        println!("installed loader: {}", dst.display());

        let fallback_dir = esp.join("EFI/BOOT");
        fs::create_dir_all(&fallback_dir).ok();
        let fallback = fallback_dir.join("BOOTX64.EFI");
        // Only take the removable-media fallback if nothing else owns it.
        if !fallback.exists() {
            fs::copy(&src, &fallback).map_err(|e| format!("copy fallback: {e}"))?;
            println!("installed fallback: {}", fallback.display());
        }

        print_register_hint(&esp);
    } else {
        println!("\n(no --efi given: config created, loader not copied)");
        println!("Re-run with --efi target/x86_64-unknown-uefi/release/myboot.efi to install the loader.");
    }
    Ok(())
}

fn print_register_hint(esp: &Path) {
    // Best-effort: figure out the block device backing the ESP for the hint.
    println!("\nTo register MyBoot with the firmware (adjust --disk/--part to your ESP):");
    println!("  sudo efibootmgr --create --disk /dev/sdX --part N \\");
    println!("      --loader '\\\\EFI\\\\MyBoot\\\\BOOTX64.EFI' --label 'MyBoot' --unicode");
    println!("(ESP mounted at {})", esp.display());
}

/// Find the ESP by looking for an `EFI/` directory under the common mount points.
/// Deliberately conservative: it never inspects partition tables or formats.
fn detect_esp() -> Option<PathBuf> {
    for cand in ESP_CANDIDATES {
        let p = PathBuf::from(cand);
        if p.join("EFI").is_dir() { return Some(p); }
    }
    None
}

fn flag_value(args: &[String], flag: &str) -> Option<String> {
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == flag { return it.next().cloned(); }
    }
    None
}
