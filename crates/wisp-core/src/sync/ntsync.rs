//! Linux NTSYNC backend.
//!
//! NTSYNC is optional: opening `/dev/ntsync` may fail on kernels without the
//! driver, so callers can fall back to Wisp's futex path.

#[cfg(target_os = "linux")]
mod linux {
    use std::io;
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};

    const CREATE_SEM: libc::c_ulong = 0x4008_4e80;
    const WAIT_ANY: libc::c_ulong = 0xc028_4e82;
    const WAIT_ALL: libc::c_ulong = 0xc028_4e83;
    const CREATE_MUTEX: libc::c_ulong = 0x4008_4e84;
    const CREATE_EVENT: libc::c_ulong = 0x4008_4e87;
    const SEM_RELEASE: libc::c_ulong = 0xc004_4e81;
    const MUTEX_UNLOCK: libc::c_ulong = 0xc008_4e85;
    const MUTEX_KILL: libc::c_ulong = 0x4004_4e86;
    const EVENT_SET: libc::c_ulong = 0x8004_4e88;
    const EVENT_RESET: libc::c_ulong = 0x8004_4e89;
    const EVENT_PULSE: libc::c_ulong = 0x8004_4e8a;

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    pub struct SemArgs {
        pub count: u32,
        pub max: u32,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    pub struct MutexArgs {
        pub owner: u32,
        pub count: u32,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    pub struct EventArgs {
        pub manual: u32,
        pub signaled: u32,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    pub struct WaitArgs {
        pub timeout: u64,
        pub objs: u64,
        pub count: u32,
        pub index: u32,
        pub flags: u32,
        pub owner: u32,
        pub alert: u32,
        pub pad: u32,
    }

    /// A single NTSYNC namespace. Object fds created from it share the same
    /// kernel-side namespace and are independently owned by `OwnedFd`.
    pub struct Ntsync {
        device: OwnedFd,
    }

    impl Ntsync {
        /// Returns `None` when `/dev/ntsync` is unavailable.
        pub fn open() -> io::Result<Self> {
            let path = c"/dev/ntsync";
            let fd = unsafe { libc::open(path.as_ptr(), libc::O_RDWR | libc::O_CLOEXEC) };
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(Self { device: unsafe { OwnedFd::from_raw_fd(fd) } })
        }

        #[inline]
        fn create(&self, request: libc::c_ulong, arg: *mut libc::c_void) -> io::Result<OwnedFd> {
            let fd = unsafe { libc::ioctl(self.device.as_raw_fd(), request, arg) };
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(unsafe { OwnedFd::from_raw_fd(fd) })
        }

        pub fn create_semaphore(&self, count: u32, max: u32) -> io::Result<OwnedFd> {
            let mut args = SemArgs { count, max };
            self.create(CREATE_SEM, (&mut args as *mut SemArgs).cast())
        }

        pub fn create_mutex(&self, owner: u32, count: u32) -> io::Result<OwnedFd> {
            let mut args = MutexArgs { owner, count };
            self.create(CREATE_MUTEX, (&mut args as *mut MutexArgs).cast())
        }

        pub fn create_event(&self, manual: bool, signaled: bool) -> io::Result<OwnedFd> {
            let mut args = EventArgs {
                manual: manual as u32,
                signaled: signaled as u32,
            };
            self.create(CREATE_EVENT, (&mut args as *mut EventArgs).cast())
        }

        #[inline]
        pub fn semaphore_release(&self, object: RawFd, count: &mut u32) -> io::Result<()> {
            ioctl_object(object, SEM_RELEASE, count as *mut u32 as *mut libc::c_void)
        }

        #[inline]
        pub fn mutex_unlock(&self, object: RawFd, args: &mut MutexArgs) -> io::Result<()> {
            ioctl_object(object, MUTEX_UNLOCK, (args as *mut MutexArgs).cast())
        }

        #[inline]
        pub fn mutex_kill(&self, object: RawFd, owner: &mut u32) -> io::Result<()> {
            ioctl_object(object, MUTEX_KILL, owner as *mut u32 as *mut libc::c_void)
        }

        #[inline]
        pub fn event_set(&self, object: RawFd) -> io::Result<u32> {
            let mut previous = 0u32;
            ioctl_object(object, EVENT_SET, &mut previous as *mut u32 as *mut libc::c_void)?;
            Ok(previous)
        }

        #[inline]
        pub fn event_reset(&self, object: RawFd) -> io::Result<u32> {
            let mut previous = 0u32;
            ioctl_object(object, EVENT_RESET, &mut previous as *mut u32 as *mut libc::c_void)?;
            Ok(previous)
        }

        #[inline]
        pub fn event_pulse(&self, object: RawFd) -> io::Result<u32> {
            let mut previous = 0u32;
            ioctl_object(object, EVENT_PULSE, &mut previous as *mut u32 as *mut libc::c_void)?;
            Ok(previous)
        }

        /// Atomically waits for one object. `timeout_ns` is an absolute
        /// CLOCK_MONOTONIC deadline; `u64::MAX` means infinite.
        pub fn wait_any(&self, objects: &[RawFd], timeout_ns: u64, owner: u32) -> io::Result<usize> {
            wait(self.device.as_raw_fd(), WAIT_ANY, objects, timeout_ns, owner)
        }

        /// Atomically waits for all objects.
        pub fn wait_all(&self, objects: &[RawFd], timeout_ns: u64, owner: u32) -> io::Result<()> {
            let _ = wait(self.device.as_raw_fd(), WAIT_ALL, objects, timeout_ns, owner)?;
            Ok(())
        }
    }

    #[inline]
    fn ioctl_object(fd: RawFd, request: libc::c_ulong, arg: *mut libc::c_void) -> io::Result<()> {
        let rc = unsafe { libc::ioctl(fd, request, arg) };
        if rc < 0 { Err(io::Error::last_os_error()) } else { Ok(()) }
    }

    fn wait(device: RawFd, request: libc::c_ulong, objects: &[RawFd], timeout_ns: u64, owner: u32) -> io::Result<usize> {
        if objects.is_empty() || objects.len() > 64 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "NTSYNC wait object count must be 1..=64"));
        }

        let mut args = WaitArgs {
            timeout: timeout_ns,
            objs: objects.as_ptr() as u64,
            count: objects.len() as u32,
            owner,
            ..WaitArgs::default()
        };

        let rc = unsafe { libc::ioctl(device, request, &mut args as *mut WaitArgs) };
        if rc < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(args.index as usize)
    }
}

#[cfg(target_os = "linux")]
pub use linux::*;

#[cfg(not(target_os = "linux"))]
pub struct Ntsync;

#[cfg(not(target_os = "linux"))]
impl Ntsync {
    pub fn open() -> std::io::Result<Self> {
        Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "NTSYNC is Linux-only"))
    }
}
