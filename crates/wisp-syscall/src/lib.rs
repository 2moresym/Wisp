use std::ffi::c_void;

/// Minimal NT-style memory API backed directly by mmap/mprotect.
#[inline]
pub unsafe fn nt_allocate_virtual_memory(size: usize, prot: i32) -> *mut c_void {
    let p = unsafe { libc::mmap(std::ptr::null_mut(), size, prot, libc::MAP_PRIVATE | libc::MAP_ANONYMOUS, -1, 0) };
    if p == libc::MAP_FAILED { std::ptr::null_mut() } else { p }
}

#[inline]
pub unsafe fn nt_protect_virtual_memory(addr: *mut c_void, size: usize, prot: i32) -> bool {
    unsafe { libc::mprotect(addr, size, prot) == 0 }
}

/// Linux clone wrapper reserved for the Wisp thread ABI layer.
pub fn create_thread(start: extern "C" fn(*mut c_void) -> *mut c_void, arg: *mut c_void) -> std::io::Result<libc::pid_t> {
    // A dedicated thread trampoline will own TLS/TEB setup before this becomes executable.
    let _ = (start, arg);
    Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "thread trampoline not installed"))
}
