//! In-memory mock implementations of the ports for host testing (report §5.4).
//! Kept separate from the trait contracts: these are test doubles, a different
//! responsibility from the port definitions themselves.
extern crate alloc;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::collections::BTreeMap;
use crate::error::{VarNamespace, IoError, IoResult};
use crate::traits::{VarStore, FileStore};

#[derive(Default)]
pub struct MockVarStore {
    global: BTreeMap<String, Vec<u8>>,
    vendor: BTreeMap<String, Vec<u8>>,
}
impl MockVarStore {
    pub fn new() -> Self { Self::default() }
    fn map(&self, ns: VarNamespace) -> &BTreeMap<String, Vec<u8>> {
        match ns { VarNamespace::Global => &self.global, VarNamespace::Vendor => &self.vendor }
    }
    fn map_mut(&mut self, ns: VarNamespace) -> &mut BTreeMap<String, Vec<u8>> {
        match ns { VarNamespace::Global => &mut self.global, VarNamespace::Vendor => &mut self.vendor }
    }
}
impl VarStore for MockVarStore {
    fn get(&self, ns: VarNamespace, name: &str) -> IoResult<Vec<u8>> {
        self.map(ns).get(name).cloned().ok_or(IoError::NotFound)
    }
    fn set(&mut self, ns: VarNamespace, name: &str, value: &[u8]) -> IoResult<()> {
        self.map_mut(ns).insert(name.to_string(), value.to_vec()); Ok(())
    }
    fn delete(&mut self, ns: VarNamespace, name: &str) -> IoResult<()> {
        self.map_mut(ns).remove(name); Ok(())
    }
    fn list(&self, ns: VarNamespace) -> IoResult<Vec<String>> {
        Ok(self.map(ns).keys().cloned().collect())
    }
}

#[derive(Default)]
pub struct MockFileStore { files: BTreeMap<String, Vec<u8>> }
impl MockFileStore {
    pub fn new() -> Self { Self::default() }
    pub fn with_file(mut self, path: &str, data: &[u8]) -> Self {
        self.files.insert(path.to_string(), data.to_vec()); self
    }
}
impl FileStore for MockFileStore {
    fn read(&self, path: &str) -> IoResult<Vec<u8>> {
        self.files.get(path).cloned().ok_or(IoError::NotFound)
    }
    fn write(&mut self, path: &str, data: &[u8]) -> IoResult<()> {
        self.files.insert(path.to_string(), data.to_vec()); Ok(())
    }
    fn append(&mut self, path: &str, data: &[u8]) -> IoResult<()> {
        self.files.entry(path.to_string()).or_default().extend_from_slice(data); Ok(())
    }
    fn exists(&self, path: &str) -> bool { self.files.contains_key(path) }
    fn list_dir(&self, path: &str) -> IoResult<Vec<String>> {
        // Normalise: ensure a single trailing separator, accept `/` or `\`.
        let sep = if path.contains('\\') { '\\' } else { '/' };
        let mut prefix = String::from(path);
        if !prefix.ends_with(sep) { prefix.push(sep); }
        let mut names: Vec<String> = Vec::new();
        for full in self.files.keys() {
            if let Some(rest) = full.strip_prefix(&prefix) {
                let child = rest.split(sep).next().unwrap_or(rest);
                if !child.is_empty() && !names.iter().any(|n| n == child) {
                    names.push(child.to_string());
                }
            }
        }
        if names.is_empty() && !self.files.keys().any(|k| k.starts_with(&prefix)) {
            return Err(IoError::NotFound);
        }
        Ok(names)
    }
}
