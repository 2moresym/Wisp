use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use wisp_core::{Module, ModuleRegistry, ModuleState, WispProcess};
use wisp_pe_loader::{dependency_paths, inspect, map_image, MappedImage, PeError, PeImage};

use crate::builtin;
use crate::exports::{parse_exports, ExportTarget};
use crate::imports::{parse_import_bindings, ImportBinding, ImportSymbol};
use crate::win32;

#[derive(Debug)]
pub enum RuntimeLoaderError {
    Io(std::io::Error),
    Pe(PeError),
    MissingModule(String),
    MissingExport { module: String, symbol: String },
    UnsupportedForwarder(String),
    Malformed(&'static str),
}

impl std::fmt::Display for RuntimeLoaderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O: {e}"),
            Self::Pe(e) => write!(f, "PE: {e}"),
            Self::MissingModule(n) => write!(f, "DLL not loaded: {n}"),
            Self::MissingExport { module, symbol } => write!(f, "export not found: {module}!{symbol}"),
            Self::UnsupportedForwarder(s) => write!(f, "unsupported export forwarder: {s}"),
            Self::Malformed(s) => write!(f, "malformed runtime loader state: {s}"),
        }
    }
}
impl std::error::Error for RuntimeLoaderError {}
impl From<std::io::Error> for RuntimeLoaderError { fn from(e: std::io::Error) -> Self { Self::Io(e) } }
impl From<PeError> for RuntimeLoaderError { fn from(e: PeError) -> Self { Self::Pe(e) } }

pub struct LoadedModule {
    pub name: String,
    pub path: PathBuf,
    pub image: Arc<MappedImage>,
    pub pe: PeImage,
}

pub struct RuntimeLoader {
    modules: RwLock<HashMap<String, Arc<LoadedModule>>>,
    registry: Arc<ModuleRegistry>,
    process: Arc<WispProcess>,
}

impl RuntimeLoader {
    pub fn new() -> Self {
        let process = WispProcess::new();
        win32::install_process(Arc::clone(&process));
        Self { modules: RwLock::new(HashMap::new()), registry: Arc::new(ModuleRegistry::new()), process }
    }

    pub fn registry(&self) -> &Arc<ModuleRegistry> { &self.registry }
    pub fn process(&self) -> &Arc<WispProcess> { &self.process }

    pub fn module(&self, name: &str) -> Option<Arc<LoadedModule>> {
        self.modules.read().expect("runtime module lock poisoned").get(&normalize_name(name)).cloned()
    }

    pub fn load_executable(&self, path: impl AsRef<Path>) -> Result<Arc<LoadedModule>, RuntimeLoaderError> {
        let main = self.load_recursive(path.as_ref(), &mut Vec::new())?;
        win32::set_current_image_base(main.image.base());
        let data = std::fs::read(path.as_ref())?;
        let bindings = parse_import_bindings(&data, &main.pe)?;
        self.bind_imports(&main, &bindings, &mut Vec::new())?;
        Ok(main)
    }

    fn load_recursive(&self, path: &Path, stack: &mut Vec<String>) -> Result<Arc<LoadedModule>, RuntimeLoaderError> {
        let pe = inspect(path)?;
        let name = path.file_name().and_then(|n| n.to_str()).ok_or(RuntimeLoaderError::Malformed("module filename"))?;
        let key = normalize_name(name);
        if let Some(existing) = self.module(&key) { return Ok(existing); }
        if stack.iter().any(|n| n.eq_ignore_ascii_case(&key)) { return Err(RuntimeLoaderError::Malformed("dependency cycle reached during load")); }
        stack.push(key.clone());

        for (dll_name, dependency) in dependency_paths(path, &pe) {
            match dependency {
                Some(dep_path) => { let _ = self.load_recursive(&dep_path, stack)?; }
                None if builtin::is_builtin(&dll_name) => {}
                None => { stack.pop(); return Err(RuntimeLoaderError::MissingModule(dll_name)); }
            }
        }

        let mapped = Arc::new(map_image(path)?);
        let data = std::fs::read(path)?;
        let exports = parse_exports(&data, &pe)?;
        let mut record = Module::new(&key, path.to_string_lossy().to_string(), mapped.base(), pe.size_of_image as u64, mapped.entry());
        for (dll_name, _) in dependency_paths(path, &pe) { record.add_dependency(dll_name); }
        if let Some(table) = &exports {
            for symbol in &table.symbols {
                let address = match &symbol.target { ExportTarget::Address { rva, .. } => mapped.base() + u64::from(*rva), ExportTarget::Forwarder(_) => 0 };
                record.add_export(wisp_core::Export { name: symbol.name.clone(), ordinal: u16::try_from(symbol.ordinal).unwrap_or(u16::MAX), address });
            }
        }
        record.state = ModuleState::Mapped;
        self.registry.insert(record);
        let loaded = Arc::new(LoadedModule { name: key.clone(), path: path.to_path_buf(), image: mapped, pe });
        self.modules.write().expect("runtime module lock poisoned").insert(key, Arc::clone(&loaded));
        stack.pop();
        Ok(loaded)
    }

    fn bind_imports(&self, main: &LoadedModule, bindings: &[ImportBinding], stack: &mut Vec<String>) -> Result<(), RuntimeLoaderError> {
        for binding in bindings {
            let address = if let Some(module) = self.module(&binding.dll) {
                self.resolve_binding(&module, &binding.symbol, stack)?
            } else {
                match &binding.symbol {
                    ImportSymbol::Name { name, .. } => builtin::address(&binding.dll, name).ok_or_else(|| RuntimeLoaderError::MissingExport { module: binding.dll.clone(), symbol: name.clone() })?,
                    ImportSymbol::Ordinal(n) => builtin::address_by_handle(builtin::module_handle(&binding.dll).unwrap_or(0), &format!("#{n}")).ok_or_else(|| RuntimeLoaderError::MissingExport { module: binding.dll.clone(), symbol: format!("#{n}") })?,
                }
            };
            patch_iat(&main.image, &main.pe, binding.iat_rva, address)?;
        }
        Ok(())
    }

    fn resolve_binding(&self, module: &LoadedModule, symbol: &ImportSymbol, stack: &mut Vec<String>) -> Result<u64, RuntimeLoaderError> {
        if stack.iter().any(|m| m.eq_ignore_ascii_case(&module.name)) { return Err(RuntimeLoaderError::UnsupportedForwarder(module.name.clone())); }
        stack.push(module.name.clone());
        let data = std::fs::read(&module.path)?;
        let table = parse_exports(&data, &module.pe)?.ok_or_else(|| RuntimeLoaderError::MissingExport { module: module.name.clone(), symbol: format_symbol(symbol) })?;
        let export = match symbol { ImportSymbol::Name { name, .. } => table.by_name(name), ImportSymbol::Ordinal(o) => table.by_ordinal(u32::from(*o)) }
            .ok_or_else(|| RuntimeLoaderError::MissingExport { module: module.name.clone(), symbol: format_symbol(symbol) })?;
        let result = match &export.target {
            ExportTarget::Address { rva, .. } => module.image.base().checked_add(u64::from(*rva)).ok_or(RuntimeLoaderError::Malformed("export address overflow")),
            ExportTarget::Forwarder(target) => {
                let (dll, sym) = target.split_once('.').ok_or_else(|| RuntimeLoaderError::UnsupportedForwarder(target.clone()))?;
                let forward = self.module(dll).ok_or_else(|| RuntimeLoaderError::MissingModule(dll.to_string()))?;
                let import = if let Some(n) = sym.strip_prefix('#') { ImportSymbol::Ordinal(n.parse::<u16>().map_err(|_| RuntimeLoaderError::UnsupportedForwarder(target.clone()))?) } else { ImportSymbol::Name { hint: 0, name: sym.to_string() } };
                self.resolve_binding(&forward, &import, stack)
            }
        };
        stack.pop();
        result
    }
}

impl Default for RuntimeLoader { fn default() -> Self { Self::new() } }

fn format_symbol(symbol: &ImportSymbol) -> String { match symbol { ImportSymbol::Name { name, .. } => name.clone(), ImportSymbol::Ordinal(n) => format!("#{n}") } }

#[inline]
pub fn normalize_name(name: &str) -> String { let name = name.replace('\\', "/"); name.rsplit('/').next().unwrap_or(&name).to_ascii_lowercase() }

fn section_protection(characteristics: u32) -> i32 {
    let mut prot = 0;
    if characteristics & 0x40000000 != 0 { prot |= libc::PROT_READ; }
    if characteristics & 0x80000000 != 0 { prot |= libc::PROT_WRITE; }
    if characteristics & 0x20000000 != 0 { prot |= libc::PROT_EXEC; }
    if prot == 0 { libc::PROT_NONE } else { prot }
}

fn patch_iat(image: &MappedImage, pe: &PeImage, rva: u32, value: u64) -> Result<(), RuntimeLoaderError> {
    let end = u64::from(rva).checked_add(8).ok_or(RuntimeLoaderError::Malformed("IAT range overflow"))?;
    if end > u64::from(pe.size_of_image) { return Err(RuntimeLoaderError::Malformed("IAT outside mapped image")); }
    let offset = usize::try_from(rva).map_err(|_| RuntimeLoaderError::Malformed("IAT address overflow"))?;
    let end_offset = offset.checked_add(8).ok_or(RuntimeLoaderError::Malformed("IAT address overflow"))?;
    let page_start = offset & !4095usize;
    let page_end = (end_offset.saturating_sub(1)) & !4095usize;
    let section = pe.sections.iter().find(|s| {
        let start = u64::from(s.virtual_address);
        let span = u64::from(s.virtual_size.max(s.raw_size));
        u64::from(rva) >= start && end <= start.saturating_add(span)
    }).ok_or(RuntimeLoaderError::Malformed("IAT not inside a section"))?;
    let restore = section_protection(section.characteristics);
    let make_rw = |page: usize| -> Result<(), RuntimeLoaderError> {
        let addr = unsafe { image.as_ptr().add(page) };
        if unsafe { libc::mprotect(addr.cast(), 4096, libc::PROT_READ | libc::PROT_WRITE) } != 0 { return Err(std::io::Error::last_os_error().into()); }
        Ok(())
    };
    make_rw(page_start)?;
    if page_end != page_start { make_rw(page_end)?; }
    unsafe { std::ptr::write_unaligned(image.as_ptr().add(offset).cast::<u64>(), value); }
    let restore_page = |page: usize| -> Result<(), RuntimeLoaderError> {
        let addr = unsafe { image.as_ptr().add(page) };
        if unsafe { libc::mprotect(addr.cast(), 4096, restore) } != 0 { return Err(std::io::Error::last_os_error().into()); }
        Ok(())
    };
    restore_page(page_start)?;
    if page_end != page_start { restore_page(page_end)?; }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_module_names() {
        assert_eq!(normalize_name("C:\\Games\\FOO.DLL"), "foo.dll");
        assert_eq!(normalize_name("foo.dll"), "foo.dll");
    }

    #[test]
    fn builtin_modules_are_recognized() {
        assert!(builtin::is_builtin("KERNEL32.DLL"));
        assert!(builtin::is_builtin("ntdll.dll"));
        assert!(!builtin::is_builtin("user32.dll"));
    }
}
