use std::ffi::c_void;
use std::sync::Arc;

use wisp_core::{Handle, ThreadState, WispProcess};

/// Minimal NT-style memory API backed directly by mmap/mprotect.
#[inline]
pub unsafe fn nt_allocate_virtual_memory(size: usize, prot: i32) -> *mut c_void {
    let p = unsafe {
        libc::mmap(
            std::ptr::null_mut(),
            size,
            prot,
            libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
            -1,
            0,
        )
    };
    if p == libc::MAP_FAILED { std::ptr::null_mut() } else { p }
}

#[inline]
pub unsafe fn nt_protect_virtual_memory(addr: *mut c_void, size: usize, prot: i32) -> bool {
    unsafe { libc::mprotect(addr, size, prot) == 0 }
}

/// Minimal thread entry ABI used internally while the Windows TEB/NT thread
/// environment is being built out.
pub type ThreadStart = extern "C" fn(*mut c_void);

/// Create a Linux-backed thread and return its Wisp handle.
///
/// The Linux thread is bootstrapped with a Wisp TEB before the callback runs.
pub fn nt_create_thread(
    process: &Arc<WispProcess>,
    start: ThreadStart,
    parameter: *mut c_void,
) -> Result<Handle, std::io::Error> {
    let parameter = parameter as usize;
    process.create_thread(move || {
        start(parameter as *mut c_void);
    })
}

/// Wait for a Wisp thread to terminate. `Ok(true)` means the handle existed.
pub fn nt_wait_for_single_object(
    process: &WispProcess,
    handle: Handle,
) -> Result<bool, Box<dyn std::any::Any + Send>> {
    match process.wait_thread(handle) {
        Some(result) => result.map(|()| true),
        None => Ok(false),
    }
}

/// Close a Wisp thread handle.
pub fn nt_close(process: &WispProcess, handle: Handle) -> bool {
    process.close_thread(handle)
}

/// Query the current emulated thread state.
pub fn nt_query_thread_state(process: &WispProcess, handle: Handle) -> Option<ThreadState> {
    process.thread_state(handle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    static RAN: AtomicBool = AtomicBool::new(false);

    extern "C" fn test_start(_parameter: *mut c_void) {
        std::thread::sleep(Duration::from_millis(2));
        RAN.store(true, Ordering::Release);
    }

    #[test]
    fn nt_thread_lifecycle_works() {
        RAN.store(false, Ordering::Release);
        let process = WispProcess::new();
        let handle = nt_create_thread(&process, test_start, std::ptr::null_mut())
            .expect("thread should be created");

        assert!(matches!(
            nt_query_thread_state(&process, handle),
            Some(ThreadState::Running)
        ));
        assert_eq!(
            nt_wait_for_single_object(&process, handle).expect("thread should exit"),
            true
        );
        assert!(RAN.load(Ordering::Acquire));
        assert_eq!(
            nt_query_thread_state(&process, handle),
            Some(ThreadState::Exited)
        );
        assert!(nt_close(&process, handle));
        assert!(!nt_close(&process, handle));
    }
}
