//! Process-local Windows module registry and dependency graph.
//!
//! This is intentionally independent of the PE byte parser. The PE loader is
//! responsible for validating metadata; this layer owns the lifetime and
//! ordering of modules once a PE has been admitted into a Wisp process.

use std::collections::{BTreeSet, HashMap};
use std::sync::RwLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleState {
    Discovered,
    Mapped,
    Initializing,
    Initialized,
    Unloading,
}

#[derive(Debug, Clone)]
pub struct Export {
    pub name: Option<String>,
    pub ordinal: u16,
    pub address: u64,
}

#[derive(Debug, Clone)]
pub struct Module {
    pub name: String,
    pub path: String,
    pub base: u64,
    pub size: u64,
    pub entry: u64,
    pub state: ModuleState,
    pub dependencies: Vec<String>,
    exports_by_name: HashMap<String, Export>,
    exports_by_ordinal: HashMap<u16, Export>,
}

impl Module {
    pub fn new(name: impl Into<String>, path: impl Into<String>, base: u64, size: u64, entry: u64) -> Self {
        Self {
            name: normalize_name(&name.into()),
            path: path.into(),
            base,
            size,
            entry,
            state: ModuleState::Discovered,
            dependencies: Vec::new(),
            exports_by_name: HashMap::new(),
            exports_by_ordinal: HashMap::new(),
        }
    }

    pub fn add_dependency(&mut self, dependency: impl Into<String>) {
        let dependency = normalize_name(&dependency.into());
        if !self.dependencies.iter().any(|d| d == &dependency) {
            self.dependencies.push(dependency);
        }
    }

    pub fn add_export(&mut self, export: Export) {
        if let Some(name) = &export.name {
            self.exports_by_name.insert(name.to_ascii_lowercase(), export.clone());
        }
        self.exports_by_ordinal.insert(export.ordinal, export);
    }

    pub fn export_by_name(&self, name: &str) -> Option<&Export> {
        self.exports_by_name.get(&name.to_ascii_lowercase())
    }

    pub fn export_by_ordinal(&self, ordinal: u16) -> Option<&Export> {
        self.exports_by_ordinal.get(&ordinal)
    }
}

#[derive(Debug, Default)]
pub struct ModuleRegistry {
    modules: RwLock<HashMap<String, Module>>,
}

impl ModuleRegistry {
    pub fn new() -> Self { Self::default() }

    pub fn insert(&self, module: Module) -> Option<Module> {
        let key = normalize_name(&module.name);
        self.modules.write().expect("module registry poisoned").insert(key, module)
    }

    pub fn remove(&self, name: &str) -> Option<Module> {
        self.modules.write().expect("module registry poisoned").remove(&normalize_name(name))
    }

    pub fn get(&self, name: &str) -> Option<Module> {
        self.modules.read().expect("module registry poisoned").get(&normalize_name(name)).cloned()
    }

    pub fn len(&self) -> usize { self.modules.read().expect("module registry poisoned").len() }

    /// Return modules in dependency-first initialization order.
    pub fn initialization_order(&self, roots: &[&str]) -> Result<Vec<String>, ModuleGraphError> {
        let modules = self.modules.read().expect("module registry poisoned");
        let mut order = Vec::new();
        let mut visiting = BTreeSet::new();
        let mut visited = BTreeSet::new();

        fn visit(
            name: &str,
            modules: &HashMap<String, Module>,
            visiting: &mut BTreeSet<String>,
            visited: &mut BTreeSet<String>,
            order: &mut Vec<String>,
        ) -> Result<(), ModuleGraphError> {
            let name = normalize_name(name);
            if visited.contains(&name) { return Ok(()); }
            if !visiting.insert(name.clone()) {
                return Err(ModuleGraphError::Cycle(name));
            }
            let module = modules.get(&name).ok_or_else(|| ModuleGraphError::Missing(name.clone()))?;
            for dependency in &module.dependencies {
                visit(dependency, modules, visiting, visited, order)?;
            }
            visiting.remove(&name);
            visited.insert(name.clone());
            order.push(name);
            Ok(())
        }

        for root in roots {
            visit(root, &modules, &mut visiting, &mut visited, &mut order)?;
        }
        Ok(order)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModuleGraphError {
    Missing(String),
    Cycle(String),
}

impl std::fmt::Display for ModuleGraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing(name) => write!(f, "missing module dependency: {name}"),
            Self::Cycle(name) => write!(f, "module dependency cycle involving: {name}"),
        }
    }
}

impl std::error::Error for ModuleGraphError {}

#[inline]
pub fn normalize_name(name: &str) -> String {
    let name = name.replace('\\', "/");
    let file = name.rsplit('/').next().unwrap_or(&name);
    file.to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dependency_order_is_stable_and_dependency_first() {
        let registry = ModuleRegistry::new();
        let mut kernel32 = Module::new("KERNEL32.DLL", "/windows/system32/kernel32.dll", 1, 2, 3);
        kernel32.add_dependency("ntdll.dll");
        let ntdll = Module::new("ntdll.dll", "/windows/system32/ntdll.dll", 4, 5, 6);
        let exe = Module::new("game.exe", "/games/game.exe", 7, 8, 9);
        registry.insert(ntdll);
        registry.insert(kernel32);
        registry.insert(exe);

        assert_eq!(
            registry.initialization_order(&["game.exe", "kernel32.dll"]).unwrap(),
            vec!["game.exe", "ntdll.dll", "kernel32.dll"]
        );
    }

    #[test]
    fn cycles_are_rejected() {
        let registry = ModuleRegistry::new();
        let mut a = Module::new("a.dll", "/a.dll", 0, 1, 0);
        a.add_dependency("b.dll");
        let mut b = Module::new("b.dll", "/b.dll", 0, 1, 0);
        b.add_dependency("a.dll");
        registry.insert(a);
        registry.insert(b);
        assert!(matches!(
            registry.initialization_order(&["a.dll"]),
            Err(ModuleGraphError::Cycle(_))
        ));
    }

    #[test]
    fn exports_support_name_and_ordinal_lookup() {
        let mut module = Module::new("x.dll", "/x.dll", 0, 1, 0);
        module.add_export(Export { name: Some("Foo".into()), ordinal: 7, address: 0x1234 });
        assert_eq!(module.export_by_name("foo").map(|e| e.address), Some(0x1234));
        assert_eq!(module.export_by_ordinal(7).map(|e| e.address), Some(0x1234));
    }
}
