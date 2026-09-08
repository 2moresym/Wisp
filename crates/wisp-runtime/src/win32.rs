use std::{ffi::c_void, sync::Arc};

use wisp_core::{Handle, WispProcess};

pub const PAGE_NOACCESS: u32 = 0x01;
pub const PAGE_READONLY: u32 = 0x02;
pub const PAGE_READWRITE: u32 = 0x04;
pub const PAGE_EXECUTE: u32 = 0x10;
pub const PAGE_EXECUTE_READ: u32 = 0x20;
pub const PAGE_EXECUTE_READWRITE: u32 = 0x40;
pub const MEM_COMMIT: u32 = 0x1000;
pub const MEM_RESERVE: u32 = 0x2000;
pub const MEM_RELEASE: u32 = 0x8000;
pub const WAIT_OBJECT_0: u32 = 0;
pub const WAIT_TIMEOUT: u32 = 0x102;
pub const WAIT_FAILED: u32 = 0xffff_ffff;

#[inline]
fn prot(p: u32) -> Option<i32> {
    Some(match p & 0xff {
        PAGE_NOACCESS => libc::PROT_NONE,
        PAGE_READONLY => libc::PROT_READ,
        PAGE_READWRITE => libc::PROT_READ | libc::PROT_WRITE,
        PAGE_EXECUTE => libc::PROT_EXEC,
        PAGE_EXECUTE_READ => libc::PROT_EXEC | libc::PROT_READ,
        PAGE_EXECUTE_READWRITE => libc::PROT_EXEC | libc::PROT_READ | libc::PROT_WRITE,
        _ => return None,
    })
}

#[inline]
pub fn get_last_error() -> u32 {
    let t = wisp_core::teb::current_teb_base();
    if t.is_null() {
        0
    } else {
        unsafe {
            std::ptr::read_unaligned(
                t.add(wisp_core::teb::TEB_LAST_ERROR_OFFSET) as *const u32,
            )
        }
    }
}

#[inline]
pub fn set_last_error(v: u32) {
    let t = wisp_core::teb::current_teb_base();
    if !t.is_null() {
        unsafe {
            std::ptr::write_unaligned(
                t.add(wisp_core::teb::TEB_LAST_ERROR_OFFSET) as *mut u32,
                v,
            );
        }
    }
}

pub fn virtual_alloc(
    address: *mut c_void,
    size: usize,
    allocation_type: u32,
    protection: u32,
) -> *mut c_void {
    if size == 0 || allocation_type & (MEM_COMMIT | MEM_RESERVE) == 0 {
        set_last_error(87);
        return std::ptr::null_mut();
    }

    let Some(p) = prot(protection) else {
        set_last_error(87);
        return std::ptr::null_mut();
    };

    let r = unsafe {
        libc::mmap(
            address,
            size,
            p,
            libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
            -1,
            0,
        )
    };

    if r == libc::MAP_FAILED {
        set_last_error(8);
        std::ptr::null_mut()
    } else {
        set_last_error(0);
        r
    }
}

pub fn virtual_protect(
    address: *mut c_void,
    size: usize,
    protection: u32,
    old: &mut u32,
) -> bool {
    if address.is_null() || size == 0 {
        set_last_error(87);
        return false;
    }

    let Some(p) = prot(protection) else {
        set_last_error(87);
        return false;
    };

    if unsafe { libc::mprotect(address, size, p) } != 0 {
        set_last_error(998);
        return false;
    }

    // Linux mprotect does not directly expose the previous protection through
    // this API. Keep the current compatibility placeholder explicit until the
    // virtual-memory manager tracks per-region protection state.
    *old = PAGE_READWRITE;
    set_last_error(0);
    true
}

pub fn tls_alloc(process: &WispProcess) -> u32 {
    process.tls_alloc().map(|v| v as u32).unwrap_or(u32::MAX)
}

pub fn tls_free(process: &WispProcess, index: u32) -> bool {
    process.tls_free(index as usize)
}

pub fn tls_get_value(process: &WispProcess, index: u32) -> usize {
    process.tls_get_value(index as usize).unwrap_or(0)
}

pub fn tls_set_value(process: &WispProcess, index: u32, value: usize) -> bool {
    process.tls_set_value(index as usize, value)
}

pub type ThreadStart = extern "C" fn(*mut c_void);

pub fn create_thread(
    process: &Arc<WispProcess>,
    start: ThreadStart,
    parameter: *mut c_void,
) -> Result<Handle, std::io::Error> {
    let parameter = parameter as usize;
    process.create_thread(move || start(parameter as *mut c_void))
}
