use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

pub mod ntsync;

#[cfg(target_os = "linux")]
#[inline]
fn futex(addr: &AtomicU32, op: i32, val: u32, timeout: *const libc::timespec) -> i32 {
    unsafe {
        libc::syscall(
            libc::SYS_futex,
            addr as *const _ as *mut u32,
            op,
            val,
            timeout,
        ) as i32
    }
}

/// Small wait-on-address primitive with a bounded userspace spin phase.
pub struct WaitWord(pub AtomicU32);

impl WaitWord {
    pub const fn new(value: u32) -> Self { Self(AtomicU32::new(value)) }

    #[inline]
    pub fn load(&self) -> u32 { self.0.load(Ordering::Acquire) }

    /// Wait until the value differs from `expected`.
    ///
    /// A futex wake is only a notification; it is not proof that the value
    /// changed. Re-check the value after every wake so spurious wakes and
    /// unrelated wakeups cannot make a caller observe a false transition.
    pub fn wait(&self, expected: u32) {
        for _ in 0..64 {
            if self.load() != expected { return; }
            std::hint::spin_loop();
        }

        #[cfg(target_os = "linux")]
        {
            const FUTEX_WAIT_PRIVATE: i32 = 128;
            loop {
                if self.load() != expected { return; }
                let _ = futex(&self.0, FUTEX_WAIT_PRIVATE, expected, std::ptr::null());
            }
        }

        #[cfg(not(target_os = "linux"))]
        loop {
            if self.load() != expected { return; }
            std::thread::yield_now();
        }
    }

    /// Wait until the value differs from `expected` or the timeout expires.
    pub fn wait_timeout(&self, expected: u32, timeout: Duration) {
        let deadline = Instant::now().checked_add(timeout);

        #[cfg(target_os = "linux")]
        {
            const FUTEX_WAIT_PRIVATE: i32 = 128;
            loop {
                if self.load() != expected { return; }

                let remaining = match deadline {
                    Some(deadline) => match deadline.checked_duration_since(Instant::now()) {
                        Some(remaining) if !remaining.is_zero() => remaining,
                        _ => return,
                    },
                    None => Duration::MAX,
                };

                let secs = remaining.as_secs();
                let tv_sec = libc::time_t::try_from(secs).unwrap_or(libc::time_t::MAX);
                let ts = libc::timespec {
                    tv_sec,
                    tv_nsec: remaining.subsec_nanos() as libc::c_long,
                };
                let _ = futex(&self.0, FUTEX_WAIT_PRIVATE, expected, &ts);
            }
        }

        #[cfg(not(target_os = "linux"))]
        while self.load() == expected {
            match deadline {
                Some(deadline) => match deadline.checked_duration_since(Instant::now()) {
                    Some(remaining) => std::thread::sleep(remaining.min(Duration::from_millis(1))),
                    None => return,
                },
                None => std::thread::yield_now(),
            }
        }
    }

    pub fn wake_all(&self) {
        #[cfg(target_os = "linux")]
        {
            const FUTEX_WAKE_PRIVATE: i32 = 129;
            let _ = futex(&self.0, FUTEX_WAKE_PRIVATE, u32::MAX, std::ptr::null());
        }
    }
}
