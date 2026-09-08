//! Windows-compatible process TLS index management and TEB slot access.
//!
//! Windows allocates TLS indices process-wide while each thread owns a value
//! for every index. Wisp mirrors that split: `TlsManager` owns index allocation,
//! while the TEB owns the per-thread values.

use std::sync::Mutex;

use crate::teb::{self, TEB_TLS_EXPANSION_SLOTS, TEB_TLS_SLOTS};

pub const TLS_MINIMUM_AVAILABLE: usize = TEB_TLS_SLOTS;
pub const TLS_MAXIMUM_AVAILABLE: usize = TEB_TLS_SLOTS + TEB_TLS_EXPANSION_SLOTS;
pub const TLS_OUT_OF_INDEXES: usize = usize::MAX;

const BITMAP_WORDS: usize = (TLS_MAXIMUM_AVAILABLE + 63) / 64;

pub struct TlsManager {
    allocated: Mutex<[u64; BITMAP_WORDS]>,
}

impl TlsManager {
    pub const fn new() -> Self {
        Self { allocated: Mutex::new([0; BITMAP_WORDS]) }
    }

    pub fn alloc(&self) -> Option<usize> {
        let mut allocated = self.allocated.lock().expect("TLS bitmap poisoned");
        for word_index in 0..BITMAP_WORDS {
            let free = !allocated[word_index];
            if free == 0 { continue; }
            let bit = free.trailing_zeros() as usize;
            let index = word_index * 64 + bit;
            if index >= TLS_MAXIMUM_AVAILABLE { break; }
            allocated[word_index] |= 1u64 << bit;
            return Some(index);
        }
        None
    }

    pub fn free(&self, index: usize) -> bool {
        if index >= TLS_MAXIMUM_AVAILABLE { return false; }
        let mut allocated = self.allocated.lock().expect("TLS bitmap poisoned");
        let word = index / 64;
        let bit = index % 64;
        let mask = 1u64 << bit;
        if allocated[word] & mask == 0 { return false; }
        allocated[word] &= !mask;
        true
    }

    #[inline]
    pub fn is_allocated(&self, index: usize) -> bool {
        if index >= TLS_MAXIMUM_AVAILABLE { return false; }
        let allocated = self.allocated.lock().expect("TLS bitmap poisoned");
        allocated[index / 64] & (1u64 << (index % 64)) != 0
    }
}

impl Default for TlsManager {
    fn default() -> Self { Self::new() }
}

/// Read the current thread's TLS value without requiring a Wisp thread handle.
pub fn current_get(index: usize) -> usize {
    let teb = teb::current_teb_base();
    if teb.is_null() || index >= TLS_MAXIMUM_AVAILABLE { return 0; }

    if index < TEB_TLS_SLOTS {
        return unsafe {
            std::ptr::read_unaligned(
                teb.add(teb::TEB_TLS_SLOTS_OFFSET + index * std::mem::size_of::<usize>())
                    as *const usize,
            )
        };
    }

    let expansion = unsafe {
        std::ptr::read_unaligned(teb.add(teb::TEB_TLS_EXPANSION_POINTER_OFFSET) as *const usize)
            as *const usize
    };
    if expansion.is_null() { return 0; }
    unsafe { std::ptr::read_unaligned(expansion.add(index - TEB_TLS_SLOTS) as *const usize) }
}

/// Set the current thread's TLS value. Returns `false` for an invalid index or
/// when the current thread does not have a Wisp TEB installed.
pub fn current_set(index: usize, value: usize) -> bool {
    let teb = teb::current_teb_base();
    if teb.is_null() || index >= TLS_MAXIMUM_AVAILABLE { return false; }

    if index < TEB_TLS_SLOTS {
        unsafe {
            std::ptr::write_unaligned(
                teb.add(teb::TEB_TLS_SLOTS_OFFSET + index * std::mem::size_of::<usize>())
                    as *mut usize,
                value,
            );
        }
        return true;
    }

    let expansion = unsafe {
        std::ptr::read_unaligned(teb.add(teb::TEB_TLS_EXPANSION_POINTER_OFFSET) as *const usize)
            as *mut usize
    };
    if expansion.is_null() { return false; }
    unsafe { std::ptr::write_unaligned(expansion.add(index - TEB_TLS_SLOTS), value); }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocator_reuses_freed_indices() {
        let manager = TlsManager::new();
        let first = manager.alloc().expect("first TLS index");
        assert!(manager.is_allocated(first));
        assert!(manager.free(first));
        assert!(!manager.is_allocated(first));
        assert_eq!(manager.alloc(), Some(first));
    }
}
