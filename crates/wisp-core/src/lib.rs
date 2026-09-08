#![deny(unsafe_op_in_unsafe_fn)]

pub mod handle;
pub mod peb;
pub mod process;
pub mod sync;
pub mod teb;
pub mod vfs;

pub use handle::{Handle, HandleTable};
pub use peb::Peb;
pub use process::{ThreadState, WispProcess};
pub use teb::{Teb, TebGuard, TEB_PEB_OFFSET, TEB_SELF_OFFSET};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct NtStatus(pub i32);

impl NtStatus {
    pub const SUCCESS: Self = Self(0);
    pub const INVALID_PARAMETER: Self = Self(-0x3fffff73);
}
