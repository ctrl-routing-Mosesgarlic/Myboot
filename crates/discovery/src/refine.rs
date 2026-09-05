//! Boot-method refinement (report §3.3). Discovery's BLS source assumes a Linux
//! kernel is an EFI stub (the common case). Here we confirm it by peeking the
//! kernel file: a valid PE/COFF EFI application → `LinuxEfiStub` (firmware loads
//! it, Secure-Boot friendly); anything else (a raw ELF/bzImage without the EFI
//! stub) → `LinuxDirect`, which routes to the `arch` boot-protocol handoff.
//!
//! This is what makes the direct-boot path reachable ONLY when it is genuinely
//! needed, and keeps the EFI-stub chainload as the default per best practice.
extern crate alloc;
use alloc::vec::Vec;
use ports::FileStore;
use graph::BootMethod;
use storage::pe::PeHeader;
use crate::RawEntry;

/// Adjust the boot method of each Linux kernel entry by inspecting the kernel
/// file. Only entries that already carry a kernel path are considered.
pub fn refine_methods<F: FileStore + ?Sized>(mut raw: Vec<RawEntry>, fs: &F) -> Vec<RawEntry> {
    for e in raw.iter_mut() {
        if e.method != BootMethod::LinuxEfiStub { continue; }
        let kernel = match &e.kernel { Some(k) => k, None => continue };
        // Read the head of the kernel to classify it (a small read is enough for
        // the PE header check). If we cannot read it, leave the assumption.
        if let Ok(bytes) = fs.read(kernel) {
            match PeHeader::parse(&bytes) {
                Ok(pe) if pe.is_efi_application => { /* confirmed EFI stub */ }
                _ => e.method = BootMethod::LinuxDirect, // not an EFI stub → direct
            }
        }
    }
    raw
}

#[cfg(test)]
mod tests {
    use super::*;
    use ports::mock::MockFileStore;
    use graph::{OsKind, EntryRole};

    fn linux_entry(kernel: &str) -> RawEntry {
        let mut e = RawEntry::new(OsKind::Linux, "Linux", "Linux", EntryRole::Default, BootMethod::LinuxEfiStub);
        e.kernel = Some(kernel.into());
        e
    }

    // A minimal valid PE/COFF EFI-application header (MZ + PE + PE32+ subsystem 10).
    fn efi_stub_bytes() -> alloc::vec::Vec<u8> {
        let mut b = alloc::vec![0u8; 0x100];
        b[0] = b'M'; b[1] = b'Z';
        let pe_off = 0x80u32;
        b[0x3C..0x40].copy_from_slice(&pe_off.to_le_bytes());
        let po = pe_off as usize;
        b[po..po+4].copy_from_slice(b"PE\0\0");
        // optional header magic PE32+ (0x20b) at po+24
        b[po+24..po+26].copy_from_slice(&0x20bu16.to_le_bytes());
        // subsystem (u16) at po+24+68 = po+92 for PE32+
        b[po+92..po+94].copy_from_slice(&10u16.to_le_bytes());
        b
    }

    #[test]
    fn efi_stub_kernel_stays_efi_stub() {
        let fs = MockFileStore::new().with_file("\\k\\bzImage", &efi_stub_bytes());
        let out = refine_methods(alloc::vec![linux_entry("\\k\\bzImage")], &fs);
        assert_eq!(out[0].method, BootMethod::LinuxEfiStub);
    }

    #[test]
    fn non_pe_kernel_becomes_direct() {
        // A raw ELF-ish kernel (no MZ) → direct boot via the arch handoff.
        let fs = MockFileStore::new().with_file("\\k\\vmlinux", b"\x7fELF and the rest");
        let out = refine_methods(alloc::vec![linux_entry("\\k\\vmlinux")], &fs);
        assert_eq!(out[0].method, BootMethod::LinuxDirect);
    }

    #[test]
    fn unreadable_kernel_keeps_the_assumption() {
        let fs = MockFileStore::new(); // file absent
        let out = refine_methods(alloc::vec![linux_entry("\\k\\missing")], &fs);
        assert_eq!(out[0].method, BootMethod::LinuxEfiStub);
    }
}
