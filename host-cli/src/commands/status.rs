//! `myboot status` — show Secure Boot posture and the firmware boot order.
use crate::efivars::{self, GLOBAL_GUID};
use super::CmdResult;

pub fn run() -> CmdResult {
    if !efivars::is_available() {
        return Err("efivarfs not available (is this an EFI system? are you root?)".into());
    }

    let sb = efivars::read("SecureBoot", GLOBAL_GUID)?;
    let secure = match sb.as_deref() {
        Some([1, ..]) => "enabled",
        Some([0, ..]) => "disabled",
        _ => "unknown",
    };
    println!("Secure Boot: {secure}");

    match efivars::read("BootOrder", GLOBAL_GUID)? {
        Some(bytes) => {
            let order: Vec<String> = bytes
                .chunks_exact(2)
                .map(|c| format!("Boot{:04X}", u16::from_le_bytes([c[0], c[1]])))
                .collect();
            println!("BootOrder:   {}", order.join(" "));
        }
        None => println!("BootOrder:   (unset)"),
    }
    Ok(())
}
