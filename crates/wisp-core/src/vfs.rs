use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Lazy directory cache used to emulate Windows case-insensitive lookup on Linux.
pub struct CaseMap {
    entries: HashMap<String, PathBuf>,
}

impl CaseMap {
    pub fn new() -> Self { Self { entries: HashMap::new() } }

    pub fn insert(&mut self, real: PathBuf) {
        if let Some(name) = real.file_name().and_then(|n| n.to_str()) {
            self.entries.insert(name.to_ascii_lowercase(), real);
        }
    }

    #[inline]
    pub fn resolve(&self, requested: &str) -> Option<&Path> {
        self.entries.get(&requested.to_ascii_lowercase()).map(PathBuf::as_path)
    }
}

impl Default for CaseMap { fn default() -> Self { Self::new() } }
