use std::{fs, io, path::{Path, PathBuf}, sync::Arc};

use wisp_core::{Module, ModuleRegistry, ModuleState};
use wisp_pe_loader::{dependency_paths, inspect};

#[derive(Debug, Clone)]
pub struct ResolvedModule {
    pub name: String,
    pub path: PathBuf,
    pub image_base: u64,
    pub image_size: u64,
    pub entry: u64,
}

#[derive(Debug)]
pub enum DllError {
    Io(io::Error),
    Missing(String),
    Invalid(String),
}

impl std::fmt::Display for DllError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "DLL I/O error: {e}"),
            Self::Missing(n) => write!(f, "DLL not found: {n}"),
            Self::Invalid(e) => write!(f, "invalid DLL: {e}"),
        }
    }
}
impl std::error::Error for DllError {}
impl From<io::Error> for DllError { fn from(e: io::Error) -> Self { Self::Io(e) } }

pub struct DllLoader {
    registry: Arc<ModuleRegistry>,
    roots: Vec<PathBuf>,
}

impl DllLoader {
    pub fn new() -> Self {
        Self { registry: Arc::new(ModuleRegistry::new()), roots: Vec::new() }
    }

    pub fn with_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.roots.push(root.into());
        self
    }

    pub fn registry(&self) -> &Arc<ModuleRegistry> { &self.registry }

    pub fn resolve_path(&self, name: &str) -> Result<PathBuf, DllError> {
        let normalized = name.replace('\\', "/");
        let needle = normalized.rsplit('/').next().unwrap_or(name).to_ascii_lowercase();
        for root in &self.roots {
            let exact = root.join(name);
            if exact.is_file() { return Ok(exact); }
            let entries = match fs::read_dir(root) {
                Ok(v) => v,
                Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
                Err(e) => return Err(e.into()),
            };
            for entry in entries.flatten() {
                if entry.file_name().to_string_lossy().eq_ignore_ascii_case(&needle) && entry.path().is_file() {
                    return Ok(entry.path());
                }
            }
        }
        Err(DllError::Missing(name.to_string()))
    }

    /// Admit validated metadata into the process module registry.
    /// Mapping and entry-point execution are separate operations.
    pub fn admit(&self, path: impl AsRef<Path>) -> Result<ResolvedModule, DllError> {
        let path = path.as_ref().to_path_buf();
        let image = inspect(&path).map_err(|e| DllError::Invalid(e.to_string()))?;
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("module").to_ascii_lowercase();
        let record = Module::new(
            &name,
            path.to_string_lossy().to_string(),
            image.image_base,
            image.size_of_image as u64,
            image.image_base + image.entry_rva as u64,
        );
        self.registry.insert(record);
        Ok(ResolvedModule {
            name,
            path,
            image_base: image.image_base,
            image_size: image.size_of_image as u64,
            entry: image.image_base + image.entry_rva as u64,
        })
    }

    pub fn mark_mapped(&self, name: &str) -> Result<bool, DllError> {
        let Some(mut module) = self.registry.get(name) else { return Err(DllError::Missing(name.to_string())); };
        module.state = ModuleState::Mapped;
        self.registry.insert(module);
        Ok(true)
    }

    pub fn mark_initialized(&self, name: &str) -> Result<bool, DllError> {
        let Some(mut module) = self.registry.get(name) else { return Err(DllError::Missing(name.to_string())); };
        module.state = ModuleState::Initialized;
        self.registry.insert(module);
        Ok(true)
    }

    pub fn dependencies(&self, image_path: impl AsRef<Path>) -> Result<Vec<(String, PathBuf)>, DllError> {
        let path = image_path.as_ref();
        let image = inspect(path).map_err(|e| DllError::Invalid(e.to_string()))?;
        let mut out = Vec::new();
        for (name, found) in dependency_paths(path, &image) {
            match found {
                Some(path) => out.push((name, path)),
                None => return Err(DllError::Missing(name)),
            }
        }
        Ok(out)
    }

    pub fn initialization_order(&self, root: &str) -> Result<Vec<String>, DllError> {
        self.registry.initialization_order(&[root]).map_err(|e| DllError::Invalid(e.to_string()))
    }
}

impl Default for DllLoader { fn default() -> Self { Self::new() } }

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn path_lookup_is_case_insensitive() {
        let dir = std::env::temp_dir().join(format!("wisp-runtime-dll-{}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("KERNEL32.DLL"), b"not a PE").unwrap();
        let loader = DllLoader::new().with_root(&dir);
        assert_eq!(loader.resolve_path("kernel32.dll").unwrap(), dir.join("KERNEL32.DLL"));
        let _ = fs::remove_dir_all(dir);
    }
}
