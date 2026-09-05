//! Minimal efivarfs access (report §3.10.3). Reads/writes UEFI variables via
//! `/sys/firmware/efi/efivars/<Name>-<GUID>`. The first 4 bytes of each file are
//! the attribute word; the payload follows. This is exactly the layout the Linux
//! kernel exposes and `efibootmgr` uses.
use std::io::{self, Read, Write};
use std::path::PathBuf;

pub const GLOBAL_GUID: &str = "8be4df61-93ca-11d2-aa0d-00e098032b8c";
pub const VENDOR_GUID: &str = "6f4e0b2a-1c3d-4e5f-9a8b-0c1d2e3f4a5b";

const EFIVARS: &str = "/sys/firmware/efi/efivars";

fn path(name: &str, guid: &str) -> PathBuf {
    PathBuf::from(EFIVARS).join(format!("{name}-{guid}"))
}

/// Read a variable's payload (attribute word stripped). None if it is absent.
pub fn read(name: &str, guid: &str) -> io::Result<Option<Vec<u8>>> {
    let p = path(name, guid);
    if !p.exists() { return Ok(None); }
    let mut f = std::fs::File::open(&p)?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf)?;
    // Strip the 4-byte attributes prefix.
    Ok(Some(if buf.len() >= 4 { buf[4..].to_vec() } else { Vec::new() }))
}

/// Write a variable payload with standard NV+BS+RT attributes.
pub fn write(name: &str, guid: &str, payload: &[u8]) -> io::Result<()> {
    // NON_VOLATILE | BOOTSERVICE_ACCESS | RUNTIME_ACCESS = 0x7
    let attrs: u32 = 0x0000_0007;
    let mut data = Vec::with_capacity(4 + payload.len());
    data.extend_from_slice(&attrs.to_le_bytes());
    data.extend_from_slice(payload);
    let p = path(name, guid);
    // efivarfs files may be immutable; callers should run as root. Open with
    // create+truncate, which efivarfs handles specially.
    let mut f = std::fs::OpenOptions::new().write(true).create(true).truncate(true).open(&p)?;
    f.write_all(&data)
}

pub fn is_available() -> bool { PathBuf::from(EFIVARS).is_dir() }
