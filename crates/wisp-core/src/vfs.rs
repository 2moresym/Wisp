use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Directory-local cache used to emulate Windows case-insensitive lookup on Linux.
///
/// Exact Linux names are checked first. The folded map is populated lazily for
/// an accessed directory and can be invalidated after writes/renames.
pub struct CaseMap {
    directory: PathBuf,
    entries: HashMap<String, PathBuf>,
    populated: bool,
}

impl CaseMap {
    pub fn new() -> Self { Self::for_directory(PathBuf::new()) }

    pub fn for_directory(directory: PathBuf) -> Self {
        Self { directory, entries: HashMap::new(), populated: false }
    }

    pub fn insert(&mut self, real: PathBuf) {
        if let Some(name) = real.file_name().and_then(|n| n.to_str()) {
            self.entries.insert(fold_name(name), real);
        }
    }

    pub fn invalidate(&mut self) {
        self.entries.clear();
        self.populated = false;
    }

    pub fn populate(&mut self) -> io::Result<()> {
        if self.populated { return Ok(()); }
        self.entries.clear();
        for entry in fs::read_dir(&self.directory)? {
            self.insert(entry?.path());
        }
        self.populated = true;
        Ok(())
    }

    #[inline]
    pub fn is_populated(&self) -> bool { self.populated }

    #[inline]
    pub fn resolve(&self, requested: &str) -> Option<&Path> {
        self.entries.get(&fold_name(requested)).map(PathBuf::as_path)
    }

    pub fn resolve_exact_or_folded(&mut self, requested: &str) -> io::Result<Option<PathBuf>> {
        let exact = self.directory.join(requested);
        if exact.exists() { return Ok(Some(exact)); }
        self.populate()?;
        Ok(self.resolve(requested).map(Path::to_path_buf))
    }
}

impl Default for CaseMap { fn default() -> Self { Self::new() } }

#[inline]
fn fold_name(name: &str) -> String { name.to_lowercase() }

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        let suffix = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        std::env::temp_dir().join(format!("wisp-vfs-{suffix}"))
    }

    #[test]
    fn folded_lookup_resolves_real_name() {
        let dir = temp_dir();
        fs::create_dir(&dir).unwrap();
        let real = dir.join("Textures.DDS");
        fs::write(&real, b"fixture").unwrap();
        let mut map = CaseMap::for_directory(dir.clone());
        assert_eq!(map.resolve_exact_or_folded("textures.dds").unwrap(), Some(real));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn exact_lookup_wins() {
        let dir = temp_dir();
        fs::create_dir(&dir).unwrap();
        let exact = dir.join("Hero.dds");
        let folded = dir.join("hero.DDS");
        fs::write(&exact, b"a").unwrap();
        fs::write(&folded, b"b").unwrap();
        let mut map = CaseMap::for_directory(dir.clone());
        assert_eq!(map.resolve_exact_or_folded("Hero.dds").unwrap(), Some(exact));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn invalidate_forces_rescan() {
        let dir = temp_dir();
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("one.bin"), b"1").unwrap();
        let mut map = CaseMap::for_directory(dir.clone());
        assert!(map.resolve_exact_or_folded("ONE.BIN").unwrap().is_some());
        fs::write(dir.join("two.bin"), b"2").unwrap();
        map.invalidate();
        assert!(map.resolve_exact_or_folded("TWO.BIN").unwrap().is_some());
        let _ = fs::remove_dir_all(dir);
    }
}
