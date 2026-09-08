//! Runtime bridge for admitted PE modules and Windows-style process APIs.

mod builtin;
mod dll;
mod exports;
mod imports;
pub mod loader;
pub mod win32;

pub use builtin::{is_builtin, module_handle, KERNEL32_HANDLE, NTDLL_HANDLE};
pub use dll::{DllError, DllLoader, ResolvedModule};
pub use exports::{parse_exports, ExportSymbol, ExportTable, ExportTarget};
pub use imports::{parse_import_bindings, ImportBinding, ImportSymbol};
pub use loader::{LoadedModule, RuntimeLoader, RuntimeLoaderError};
pub use win32::{abi_address, create_thread, current_process, get_last_error, install_process, set_current_image_base, set_last_error, tls_alloc, tls_free, tls_get_value, tls_set_value, virtual_alloc, virtual_protect, WAIT_FAILED, WAIT_INFINITE, WAIT_OBJECT_0, WAIT_TIMEOUT};
