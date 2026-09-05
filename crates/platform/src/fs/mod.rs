//! `FileStore` over the UEFI Simple File System protocol (report §4.4). Reads and
//! writes files by absolute EFI path and lists directories (backing discovery's
//! ESP/BLS/bootspec scans).
//!
//! STATELESS design: the firmware wants a `&mut` volume handle to open files, but
//! the port's read methods take `&self`. Each call reopens a fresh root, so the
//! adapter needs no interior mutability and stays simple and correct (SRP). The
//! root-based helpers here are reused by `volume::Volume` (a store bound to a
//! specific filesystem handle) so multi-volume discovery shares one code path.
extern crate alloc;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use uefi::boot;
use uefi::proto::media::file::{File, FileAttribute, FileInfo, FileMode, FileType, Directory};
use uefi::{CString16, Status};
use ports::{FileStore, IoError, IoResult};

/// Zero-sized, stateless adapter over the loaded image's ESP volume.
pub struct UefiFileStore;

impl UefiFileStore {
    pub fn new() -> Self { UefiFileStore }
}
impl Default for UefiFileStore { fn default() -> Self { Self::new() } }

pub(crate) fn to_cpath(path: &str) -> Result<CString16, IoError> {
    let norm: String = path.chars().map(|c| if c == '/' { '\\' } else { c }).collect();
    CString16::try_from(norm.as_str()).map_err(|_| IoError::Other("bad path".to_string()))
}

pub(crate) fn map_err(s: Status) -> IoError {
    match s {
        Status::NOT_FOUND => IoError::NotFound,
        Status::ACCESS_DENIED | Status::WRITE_PROTECTED => IoError::AccessDenied,
        Status::OUT_OF_RESOURCES | Status::VOLUME_FULL => IoError::TooLarge,
        Status::DEVICE_ERROR | Status::MEDIA_CHANGED | Status::NO_MEDIA => IoError::DeviceError,
        other => IoError::Other(alloc::format!("uefi status {other:?}")),
    }
}

/// Open the loaded image's ESP volume root (fresh each call — see module note).
fn open_image_root() -> IoResult<Directory> {
    let mut sfs = boot::get_image_file_system(boot::image_handle())
        .map_err(|e| map_err(e.status()))?;
    sfs.open_volume().map_err(|e| map_err(e.status()))
}

// --- root-based helpers, shared by UefiFileStore and volume::Volume ---

pub(crate) fn read_in(root: &mut Directory, path: &str) -> IoResult<Vec<u8>> {
    let cpath = to_cpath(path)?;
    let handle = root.open(&cpath, FileMode::Read, FileAttribute::empty())
        .map_err(|e| map_err(e.status()))?;
    match handle.into_type().map_err(|e| map_err(e.status()))? {
        FileType::Regular(mut f) => {
            let info = f.get_boxed_info::<FileInfo>().map_err(|e| map_err(e.status()))?;
            let size = info.file_size() as usize;
            let mut data = alloc::vec![0u8; size];
            let n = f.read(&mut data).map_err(|e| map_err(e.status()))?;
            data.truncate(n);
            Ok(data)
        }
        FileType::Dir(_) => Err(IoError::Other("path is a directory".to_string())),
    }
}

pub(crate) fn write_in(root: &mut Directory, path: &str, data: &[u8], append: bool) -> IoResult<()> {
    let cpath = to_cpath(path)?;
    let handle = root.open(&cpath, FileMode::CreateReadWrite, FileAttribute::empty())
        .map_err(|e| map_err(e.status()))?;
    match handle.into_type().map_err(|e| map_err(e.status()))? {
        FileType::Regular(mut f) => {
            if append {
                let info = f.get_boxed_info::<FileInfo>().map_err(|e| map_err(e.status()))?;
                f.set_position(info.file_size()).map_err(|e| map_err(e.status()))?;
            }
            f.write(data).map_err(|_| IoError::DeviceError)?;
            f.flush().map_err(|e| map_err(e.status()))?;
            Ok(())
        }
        FileType::Dir(_) => Err(IoError::Other("path is a directory".to_string())),
    }
}

pub(crate) fn exists_in(root: &mut Directory, path: &str) -> bool {
    match to_cpath(path) {
        Ok(cpath) => root.open(&cpath, FileMode::Read, FileAttribute::empty()).is_ok(),
        _ => false,
    }
}

pub(crate) fn list_dir_in(root: &mut Directory, path: &str) -> IoResult<Vec<String>> {
    let cpath = to_cpath(path)?;
    let handle = root.open(&cpath, FileMode::Read, FileAttribute::empty())
        .map_err(|e| map_err(e.status()))?;
    let mut dir = match handle.into_type().map_err(|e| map_err(e.status()))? {
        FileType::Dir(d) => d,
        FileType::Regular(_) => return Err(IoError::Other("path is a file".to_string())),
    };
    let mut names = Vec::new();
    let mut buf = alloc::vec![0u8; 512];
    loop {
        match dir.read_entry(&mut buf) {
            Ok(Some(info)) => {
                let name = info.file_name().to_string();
                if name != "." && name != ".." { names.push(name); }
            }
            Ok(None) => break,
            Err(e) => {
                if e.status() == Status::BUFFER_TOO_SMALL { buf.resize(buf.len() * 2, 0); continue; }
                return Err(map_err(e.status()));
            }
        }
    }
    Ok(names)
}

impl FileStore for UefiFileStore {
    fn read(&self, path: &str) -> IoResult<Vec<u8>> { read_in(&mut open_image_root()?, path) }
    fn write(&mut self, path: &str, data: &[u8]) -> IoResult<()> { write_in(&mut open_image_root()?, path, data, false) }
    fn append(&mut self, path: &str, data: &[u8]) -> IoResult<()> { write_in(&mut open_image_root()?, path, data, true) }
    fn exists(&self, path: &str) -> bool {
        match open_image_root() { Ok(mut r) => exists_in(&mut r, path), Err(_) => false }
    }
    fn list_dir(&self, path: &str) -> IoResult<Vec<String>> { list_dir_in(&mut open_image_root()?, path) }
}
