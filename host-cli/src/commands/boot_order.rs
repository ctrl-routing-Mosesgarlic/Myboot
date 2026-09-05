//! `myboot boot-order` — print the firmware BootOrder as Boot#### option numbers.
use crate::efivars::{self, GLOBAL_GUID};
use super::CmdResult;

pub fn run() -> CmdResult {
    match efivars::read("BootOrder", GLOBAL_GUID)? {
        Some(bytes) => {
            for c in bytes.chunks_exact(2) {
                println!("Boot{:04X}", u16::from_le_bytes([c[0], c[1]]));
            }
            Ok(())
        }
        None => Err("BootOrder is not set".into()),
    }
}
