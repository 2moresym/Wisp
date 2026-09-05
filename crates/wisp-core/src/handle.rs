use std::sync::atomic::{AtomicU32, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct Handle(pub u32);

pub struct HandleTable {
    next: AtomicU32,
}

impl HandleTable {
    pub const fn new() -> Self { Self { next: AtomicU32::new(4) } }

    #[inline(always)]
    pub fn reserve(&self) -> Handle {
        Handle(self.next.fetch_add(4, Ordering::Relaxed))
    }
}

impl Default for HandleTable { fn default() -> Self { Self::new() } }
