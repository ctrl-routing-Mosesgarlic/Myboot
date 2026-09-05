//! ports — the driven-port (outbound) interfaces the domain core owns
//! (ADR 0003; report §3.4, §3.9). This is the hexagonal boundary: the core
//! defines HOW it needs to touch the outside world; the `platform` adapter
//! implements these against uefi-rs. Nothing here knows about uefi.
//!
//! Split by responsibility: `error` (the uniform error + namespace), `traits`
//! (the port contracts), `mock` (in-memory test doubles). All are re-exported so
//! consumers keep using a flat `ports::X` path.
#![no_std]
#![forbid(unsafe_code)]
extern crate alloc;

mod error;
mod traits;

pub use error::{VarNamespace, IoError, IoResult};
pub use traits::{VarStore, FileStore};

#[cfg(any(test, feature = "mock"))]
pub mod mock;

#[cfg(test)]
mod tests {
    use super::*;
    use super::mock::{MockVarStore, MockFileStore};
    extern crate alloc;
    use alloc::string::ToString;

    #[test]
    fn var_store_roundtrip_and_namespaces_are_isolated() {
        let mut vs = MockVarStore::new();
        vs.set(VarNamespace::Global, "BootOrder", &[0, 0, 1, 0]).unwrap();
        vs.set(VarNamespace::Vendor, "state", &[42]).unwrap();
        assert_eq!(vs.get(VarNamespace::Global, "BootOrder").unwrap(), [0u8,0,1,0].to_vec());
        assert_eq!(vs.get(VarNamespace::Global, "state"), Err(IoError::NotFound));
        assert_eq!(vs.get(VarNamespace::Vendor, "state").unwrap(), [42u8].to_vec());
        vs.delete(VarNamespace::Vendor, "state").unwrap();
        vs.delete(VarNamespace::Vendor, "state").unwrap(); // idempotent
        assert_eq!(vs.get(VarNamespace::Vendor, "state"), Err(IoError::NotFound));
    }

    #[test]
    fn file_store_lists_immediate_children() {
        let fs = MockFileStore::new()
            .with_file("\\loader\\entries\\a.conf", b"x")
            .with_file("\\loader\\entries\\b.conf", b"y")
            .with_file("\\loader\\entries\\sub\\c.conf", b"z");
        let mut names = fs.list_dir("\\loader\\entries").unwrap();
        names.sort();
        assert_eq!(names, alloc::vec!["a.conf".to_string(), "b.conf".to_string(), "sub".to_string()]);
        assert_eq!(fs.list_dir("\\nope"), Err(IoError::NotFound));
    }

    #[test]
    fn file_store_append_creates_then_grows() {
        let mut fs = MockFileStore::new();
        assert!(!fs.exists("\\EFI\\MyBoot\\log"));
        fs.append("\\EFI\\MyBoot\\log", b"a").unwrap();
        fs.append("\\EFI\\MyBoot\\log", b"b").unwrap();
        assert_eq!(fs.read("\\EFI\\MyBoot\\log").unwrap(), b"ab".to_vec());
        assert!(fs.exists("\\EFI\\MyBoot\\log"));
    }
}
