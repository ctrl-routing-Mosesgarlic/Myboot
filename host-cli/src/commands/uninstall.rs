//! `myboot uninstall` — remove MyBoot from the ESP, leaving other OSes bootable
//! through their own loaders (release-certification requirement). Conservative:
//! it removes ONLY MyBoot's own directory and, if MyBoot owns the removable
//! fallback, restores nothing automatically — it warns instead, because another
//! loader may need to reclaim it. It never touches other vendors' files.
use super::CmdResult;
use std::fs;
use std::path::PathBuf;

const ESP_CANDIDATES: &[&str] = &["/boot/efi", "/boot", "/efi"];

pub fn run(args: &[String]) -> CmdResult {
    let esp = match flag_value(args, "--esp") {
        Some(p) => PathBuf::from(p),
        None => ESP_CANDIDATES.iter().map(PathBuf::from).find(|p| p.join("EFI/MyBoot").is_dir())
            .ok_or("could not find EFI/MyBoot on any ESP; pass --esp <mountpoint>")?,
    };

    let dir = esp.join("EFI/MyBoot");
    if dir.is_dir() {
        fs::remove_dir_all(&dir).map_err(|e| format!("remove {}: {e}", dir.display()))?;
        println!("removed {}", dir.display());
    } else {
        println!("nothing to remove at {}", dir.display());
    }

    println!("\nIf you registered MyBoot with the firmware, remove that entry too:");
    println!("  efibootmgr            # find the MyBoot Boot#### number");
    println!("  sudo efibootmgr -b <NNNN> -B   # delete it");
    println!("\nOther operating systems remain bootable through their own loaders.");
    Ok(())
}

fn flag_value(args: &[String], flag: &str) -> Option<String> {
    let mut it = args.iter();
    while let Some(a) = it.next() { if a == flag { return it.next().cloned(); } }
    None
}
