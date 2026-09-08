//! Runtime bridge for admitted PE modules and Windows-style process APIs.

mod dll;
mod exports;
mod imports;
pub mod loader;
pub mod win32;

pub use dll::{DllError, DllLoader, ResolvedModule};
pub use exports::{parse_exports, ExportSymbol, ExportTable, ExportTarget};
pub use imports::{parse_import_bindings, ImportBinding, ImportSymbol};
pub use loader::{LoadedModule, RuntimeLoader, RuntimeLoaderError};
pub use win32::{create_thread, get_last_error, set_last_error, tls_alloc, tls_free, tls_get_value, tls_set_value, virtual_alloc, virtual_protect};
