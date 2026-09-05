use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

#[cfg(target_os = "linux")]
#[inline]
fn futex(addr: &AtomicU32, op: i32, val: u32, timeout: *const libc::timespec) -> i32 {
    unsafe { libc::syscall(libc::SYS_futex, addr as *const _ as *mut u32, op, val, timeout) as i32 }
}

/// Small wait-on-address primitive. Callers spin briefly before entering the kernel.
pub struct WaitWord(pub AtomicU32);

impl WaitWord {
    pub const fn new(value: u32) -> Self { Self(AtomicU32::new(value)) }

    #[inline(always)]
    pub fn load(&self) -> u32 { self.0.load(Ordering::Acquire) }

    pub fn wait(&self, expected: u32) {
        for _ in 0..64 {
            if self.load() != expected { return; }
            std::hint::spin_loop();
        }
        #[cfg(target_os = "linux")]
        {
            const FUTEX_WAIT_PRIVATE: i32 = 128;
            let _ = futex(&self.0, FUTEX_WAIT_PRIVATE, expected, std::ptr::null());
        }
    }

    pub fn wake_all(&self) {
        #[cfg(target_os = "linux")]
        {
            const FUTEX_WAKE_PRIVATE: i32 = 129;
            let _ = futex(&self.0, FUTEX_WAKE_PRIVATE, u32::MAX, std::ptr::null());
        }
    }

    pub fn wait_timeout(&self, expected: u32, timeout: Duration) {
        if self.load() != expected { return; }
        #[cfg(target_os = "linux")]
        {
            const FUTEX_WAIT_PRIVATE: i32 = 128;
            let ts = libc::timespec { tv_sec: timeout.as_secs() as _, tv_nsec: timeout.subsec_nanos() as _ };
            let _ = futex(&self.0, FUTEX_WAIT_PRIVATE, expected, &ts);
        }
    }
}
