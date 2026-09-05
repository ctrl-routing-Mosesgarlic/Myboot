//! Boot Loader Specification Type #1 entries — the `\loader\entries\*.conf`
//! format used by systemd-boot and others (report §3.3). Each `.conf` is a set
//! of `key value` lines; we read `title`, `version`, `linux`, `initrd`,
//! `options`, and `sort-key`. Parsing is total: a malformed file is skipped.
extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;
use ports::FileStore;
use graph::{OsKind, EntryRole, BootMethod};
use crate::RawEntry;

pub const ENTRIES_DIR: &str = "\\loader\\entries";

pub fn discover<F: FileStore + ?Sized>(fs: &F) -> Vec<RawEntry> {
    let mut out = Vec::new();
    let names = match fs.list_dir(ENTRIES_DIR) { Ok(n) => n, Err(_) => return out };
    for name in names {
        if !name.ends_with(".conf") { continue; }
        let mut path = String::from(ENTRIES_DIR);
        path.push('\\');
        path.push_str(&name);
        let bytes = match fs.read(&path) { Ok(b) => b, Err(_) => continue };
        if let Some(e) = parse_conf(&bytes) { out.push(e); }
    }
    out
}

/// Parse one BLS conf. Returns None if it lacks a usable kernel (`linux`).
pub fn parse_conf(bytes: &[u8]) -> Option<RawEntry> {
    let text = core::str::from_utf8(bytes).ok()?;
    let mut title: Option<String> = None;
    let mut version: Option<String> = None;
    let mut linux: Option<String> = None;
    let mut initrd: Option<String> = None;
    let mut options: Option<String> = None;

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        // split on the first run of whitespace: `key value...`
        let (key, val) = match line.split_once(char::is_whitespace) {
            Some((k, v)) => (k, v.trim()),
            None => continue,
        };
        match key {
            "title" => title = Some(val.into()),
            "version" => version = Some(val.into()),
            "linux" => linux = Some(to_efi_path(val)),
            "initrd" => initrd = Some(to_efi_path(val)),
            "options" => options = Some(val.into()),
            _ => {} // ignore machine-id, sort-key, architecture, devicetree, ...
        }
    }

    let linux = linux?; // a Type #1 entry without a kernel is not bootable
    let title = title.or(version.clone()).unwrap_or_else(|| String::from("Linux"));
    let role = match &version { Some(v) => EntryRole::Kernel(v.clone()), None => EntryRole::Default };

    let mut e = RawEntry::new(OsKind::Linux, "Linux", title, role, BootMethod::LinuxEfiStub);
    e.kernel = Some(linux);
    e.initrd = initrd;
    e.cmdline = options;
    Some(e)
}

/// BLS paths are ESP-relative with `/`; normalise to EFI `\` form.
fn to_efi_path(p: &str) -> String {
    let mut s = String::with_capacity(p.len());
    for c in p.chars() { s.push(if c == '/' { '\\' } else { c }); }
    if !s.starts_with('\\') { s.insert(0, '\\'); }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use ports::mock::MockFileStore;

    const CONF: &str = "title Fedora 39\nversion 6.6.0\nlinux /6a/linux\ninitrd /6a/initrd\noptions root=UUID=abc ro\n";

    #[test]
    fn parses_a_type1_entry() {
        let e = parse_conf(CONF.as_bytes()).unwrap();
        assert_eq!(e.kind, OsKind::Linux);
        assert_eq!(e.title, "Fedora 39");
        assert_eq!(e.kernel.as_deref(), Some("\\6a\\linux"));
        assert_eq!(e.initrd.as_deref(), Some("\\6a\\initrd"));
        assert_eq!(e.cmdline.as_deref(), Some("root=UUID=abc ro"));
        assert_eq!(e.role, EntryRole::Kernel("6.6.0".into()));
    }

    #[test]
    fn entry_without_kernel_is_rejected() {
        assert!(parse_conf(b"title Broken\noptions ro\n").is_none());
    }

    #[test]
    fn scans_entries_directory() {
        let fs = MockFileStore::new()
            .with_file("\\loader\\entries\\a.conf", CONF.as_bytes())
            .with_file("\\loader\\entries\\README", b"ignore me");
        let entries = discover(&fs);
        assert_eq!(entries.len(), 1);
    }
}
