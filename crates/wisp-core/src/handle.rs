use std::collections::VecDeque;
use std::sync::{atomic::{AtomicU32, Ordering}, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct Handle(pub u32);

/// Process-local Windows-style handle allocator.
///
/// Handles are opaque to callers. Released values are recycled before the
/// allocator advances its monotonic cursor. Handle `0` is reserved as null.
pub struct HandleTable {
    next: AtomicU32,
    free: Mutex<VecDeque<Handle>>,
}

impl HandleTable {
    pub const fn new() -> Self {
        Self {
            next: AtomicU32::new(4),
            free: Mutex::new(VecDeque::new()),
        }
    }

    /// Reserve a handle without allowing the 32-bit handle space to wrap.
    pub fn reserve(&self) -> Option<Handle> {
        if let Some(handle) = self.free.lock().expect("handle free-list poisoned").pop_front() {
            return Some(handle);
        }

        let mut current = self.next.load(Ordering::Relaxed);
        loop {
            if current == 0 || current > u32::MAX - 3 {
                return None;
            }
            let next = current + 4;
            match self.next.compare_exchange_weak(
                current,
                next,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => return Some(Handle(current)),
                Err(observed) => current = observed,
            }
        }
    }

    /// Return a valid non-null handle number to the free list.
    pub fn release(&self, handle: Handle) {
        if handle.0 == 0 || handle.0 % 4 != 0 {
            return;
        }
        self.free
            .lock()
            .expect("handle free-list poisoned")
            .push_back(handle);
    }
}

impl Default for HandleTable {
    fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_recycle_after_release() {
        let table = HandleTable::new();
        let first = table.reserve().expect("first handle");
        let second = table.reserve().expect("second handle");
        assert_ne!(first, second);
        table.release(first);
        assert_eq!(table.reserve(), Some(first));
        assert_ne!(table.reserve(), Some(first));
    }

    #[test]
    fn null_and_misaligned_handles_are_not_recycled() {
        let table = HandleTable::new();
        table.release(Handle(0));
        table.release(Handle(3));
        assert_eq!(table.reserve(), Some(Handle(4)));
    }
}
