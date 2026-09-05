#![deny(unsafe_op_in_unsafe_fn)]

pub mod handle;
pub mod sync;
pub mod vfs;

pub use handle::{Handle, HandleTable};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct NtStatus(pub i32);

impl NtStatus {
    pub const SUCCESS: Self = Self(0);
    pub const INVALID_PARAMETER: Self = Self(-0x3fffff73);
}
