//! x86-64 Windows thread environment bootstrap.
//!
//! Windows x64 exposes the current TEB through GS. Wisp uses Linux's
//! ARCH_SET_GS/ARCH_GET_GS interface to install a per-thread TEB while PE code
//! is executing. Only compatibility-contract fields are represented directly;
//! the remaining Windows TEB stays opaque until a field is required.

use std::{io, mem::size_of, ptr};

use crate::Peb;

pub const TEB_SIZE: usize = 0x1800;
pub const TEB_STACK_BASE_OFFSET: usize = 0x08;
pub const TEB_STACK_LIMIT_OFFSET: usize = 0x10;
pub const TEB_SELF_OFFSET: usize = 0x30;
pub const TEB_CLIENT_ID_PROCESS_OFFSET: usize = 0x40;
pub const TEB_CLIENT_ID_THREAD_OFFSET: usize = 0x48;
pub const TEB_TLS_POINTER_OFFSET: usize = 0x58;
pub const TEB_PEB_OFFSET: usize = 0x60;
pub const TEB_LAST_ERROR_OFFSET: usize = 0x68;
pub const TEB_TLS_SLOTS_OFFSET: usize = 0x1480;
pub const TEB_TLS_SLOTS: usize = 64;
pub const TEB_TLS_EXPANSION_POINTER_OFFSET: usize = 0x1780;
pub const TEB_TLS_EXPANSION_SLOTS: usize = 1024;

const ARCH_SET_GS: libc::c_long = 0x1001;
const ARCH_GET_GS: libc::c_long = 0x1004;

#[repr(align(16))]
struct TebBytes([u8; TEB_SIZE]);

pub struct Teb {
    bytes: Box<TebBytes>,
    tls_expansion: Box<[usize; TEB_TLS_EXPANSION_SLOTS]>,
}

pub struct TebGuard {
    previous_gs: usize,
    active: bool,
}

impl Teb {
    pub fn new(peb: &Peb, process_id: u64, thread_id: u64) -> Self {
        let mut teb = Self {
            bytes: Box::new(TebBytes([0; TEB_SIZE])),
            tls_expansion: Box::new([0; TEB_TLS_EXPANSION_SLOTS]),
        };
        let base = teb.as_ptr();
        let tls_slots = teb.tls_slots_ptr();
        let tls_expansion = teb.tls_expansion.as_mut_ptr() as *mut u8;
        teb.write_ptr(TEB_SELF_OFFSET, base);
        teb.write_ptr(TEB_PEB_OFFSET, peb.as_ptr());
        teb.write_u64(TEB_CLIENT_ID_PROCESS_OFFSET, process_id);
        teb.write_u64(TEB_CLIENT_ID_THREAD_OFFSET, thread_id);
        teb.write_ptr(TEB_TLS_POINTER_OFFSET, tls_slots);
        teb.write_ptr(TEB_TLS_EXPANSION_POINTER_OFFSET, tls_expansion);
        teb
    }

    #[inline]
    pub fn as_ptr(&self) -> *mut u8 { self.bytes.0.as_ptr() as *mut u8 }

    #[inline]
    pub fn tls_slots_ptr(&self) -> *mut u8 {
        unsafe { self.as_ptr().add(TEB_TLS_SLOTS_OFFSET) }
    }

    #[inline]
    pub fn set_stack_bounds(&mut self, base: usize, limit: usize) {
        self.write_ptr(TEB_STACK_BASE_OFFSET, base as *mut u8);
        self.write_ptr(TEB_STACK_LIMIT_OFFSET, limit as *mut u8);
    }

    #[inline]
    pub fn last_error(&self) -> u32 { self.read_u32(TEB_LAST_ERROR_OFFSET) }

    #[inline]
    pub fn set_last_error(&mut self, value: u32) { self.write_u32(TEB_LAST_ERROR_OFFSET, value); }

    #[inline]
    pub fn tls_slot(&self, index: usize) -> Option<usize> {
        (index < TEB_TLS_SLOTS).then(|| unsafe {
            ptr::read_unaligned(
                self.as_ptr().add(TEB_TLS_SLOTS_OFFSET + index * size_of::<usize>()) as *const usize,
            )
        })
    }

    #[inline]
    pub fn set_tls_slot(&mut self, index: usize, value: usize) -> bool {
        if index >= TEB_TLS_SLOTS { return false; }
        unsafe {
            ptr::write_unaligned(
                self.as_ptr().add(TEB_TLS_SLOTS_OFFSET + index * size_of::<usize>()) as *mut usize,
                value,
            );
        }
        true
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    pub fn install(&self) -> io::Result<TebGuard> {
        let previous_gs = get_gs()?;
        let rc = unsafe { libc::syscall(libc::SYS_arch_prctl, ARCH_SET_GS, self.as_ptr() as usize) };
        if rc != 0 { return Err(io::Error::last_os_error()); }
        Ok(TebGuard { previous_gs, active: true })
    }

    #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
    pub fn install(&self) -> io::Result<TebGuard> {
        let _ = self;
        Err(io::Error::new(io::ErrorKind::Unsupported, "Wisp x64 TEB requires Linux x86-64"))
    }

    #[inline]
    fn read_u32(&self, offset: usize) -> u32 {
        debug_assert!(offset + 4 <= TEB_SIZE);
        unsafe { ptr::read_unaligned(self.as_ptr().add(offset) as *const u32) }
    }

    #[inline]
    fn write_u32(&mut self, offset: usize, value: u32) {
        debug_assert!(offset + 4 <= TEB_SIZE);
        unsafe { ptr::write_unaligned(self.as_ptr().add(offset) as *mut u32, value) }
    }

    #[inline]
    fn write_u64(&mut self, offset: usize, value: u64) {
        debug_assert!(offset + 8 <= TEB_SIZE);
        unsafe { ptr::write_unaligned(self.as_ptr().add(offset) as *mut u64, value) }
    }

    #[inline]
    fn write_ptr(&mut self, offset: usize, value: *mut u8) { self.write_u64(offset, value as usize as u64); }
}

impl TebGuard {
    #[inline]
    pub fn is_active(&self) -> bool { self.active }
}

impl Drop for TebGuard {
    fn drop(&mut self) {
        if !self.active { return; }
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        let _ = unsafe { libc::syscall(libc::SYS_arch_prctl, ARCH_SET_GS, self.previous_gs) };
        self.active = false;
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn get_gs() -> io::Result<usize> {
    let mut value = 0usize;
    let rc = unsafe { libc::syscall(libc::SYS_arch_prctl, ARCH_GET_GS, &mut value as *mut usize) };
    if rc != 0 { Err(io::Error::last_os_error()) } else { Ok(value) }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub fn current_teb_base() -> *mut u8 { get_gs().unwrap_or(0) as *mut u8 }

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub fn current_peb_base() -> *mut u8 {
    let teb = current_teb_base();
    if teb.is_null() { return ptr::null_mut(); }
    unsafe { ptr::read_unaligned(teb.add(TEB_PEB_OFFSET) as *const usize) as *mut u8 }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub fn current_stack_bounds() -> Option<(usize, usize)> {
    let mut attr = unsafe { std::mem::zeroed::<libc::pthread_attr_t>() };
    let rc = unsafe { libc::pthread_getattr_np(libc::pthread_self(), &mut attr) };
    if rc != 0 { return None; }
    let mut base = ptr::null_mut();
    let mut size = 0usize;
    let stack_rc = unsafe { libc::pthread_attr_getstack(&attr, &mut base, &mut size) };
    let _ = unsafe { libc::pthread_attr_destroy(&mut attr) };
    if stack_rc != 0 || base.is_null() || size == 0 { return None; }
    Some((base as usize + size, base as usize))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_offsets_are_expected() {
        assert_eq!(TEB_SELF_OFFSET, 0x30);
        assert_eq!(TEB_TLS_POINTER_OFFSET, 0x58);
        assert_eq!(TEB_PEB_OFFSET, 0x60);
        assert_eq!(TEB_TLS_SLOTS_OFFSET, 0x1480);
        assert_eq!(TEB_TLS_EXPANSION_POINTER_OFFSET, 0x1780);
        assert_eq!(TEB_SIZE, 0x1800);
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    #[test]
    fn installs_and_restores_gs() {
        let peb = Peb::new();
        let teb = Teb::new(&peb, 7, 9);
        let before = get_gs().expect("ARCH_GET_GS should work");
        let guard = teb.install().expect("ARCH_SET_GS should work");
        assert_eq!(current_teb_base(), teb.as_ptr());
        assert_eq!(current_peb_base(), peb.as_ptr());
        assert!(guard.is_active());
        drop(guard);
        assert_eq!(get_gs().expect("ARCH_GET_GS should work"), before);
    }
}
