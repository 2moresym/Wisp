#![deny(unsafe_op_in_unsafe_fn)]

pub mod handle;
pub mod loader;
pub mod module;
pub mod peb;
pub mod process;
pub mod sync;
pub mod teb;
pub mod tls;
pub mod vfs;

pub use handle::{Handle, HandleTable};
pub use loader::{LoaderError, LoaderPhase, LoaderState};
pub use module::{Export, Module, ModuleGraphError, ModuleRegistry, ModuleState};
pub use peb::Peb;
pub use process::{ThreadState, WispProcess};
pub use teb::{Teb, TebGuard, TEB_PEB_OFFSET, TEB_SELF_OFFSET};
pub use tls::{TlsManager, TLS_MAXIMUM_AVAILABLE, TLS_MINIMUM_AVAILABLE, TLS_OUT_OF_INDEXES};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct NtStatus(pub i32);

impl NtStatus {
    pub const SUCCESS: Self = Self(0);
    // Canonical Windows NTSTATUS values.
    pub const INVALID_PARAMETER: Self = Self(-0x3fff_fff3);
    pub const NOT_IMPLEMENTED: Self = Self(-0x3fff_fffe);
    pub const OBJECT_NAME_NOT_FOUND: Self = Self(-0x3fff_ffcc);
    pub const ACCESS_DENIED: Self = Self(-0x3fff_ffde);
}

#[cfg(test)]
mod tests {
    use super::NtStatus;

    #[test]
    fn ntstatus_values_match_windows() {
        assert_eq!(NtStatus::INVALID_PARAMETER.0 as u32, 0xC000_000D);
        assert_eq!(NtStatus::NOT_IMPLEMENTED.0 as u32, 0xC000_0002);
        assert_eq!(NtStatus::OBJECT_NAME_NOT_FOUND.0 as u32, 0xC000_0034);
        assert_eq!(NtStatus::ACCESS_DENIED.0 as u32, 0xC000_0022);
    }
}
