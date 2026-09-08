use std::ffi::{c_char, c_void, CStr};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

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
pub const WAIT_INFINITE: u32 = 0xffff_ffff;
pub const TLS_OUT_OF_INDEXES: u32 = 0xffff_ffff;
pub const ERROR_SUCCESS: u32 = 0;
pub const ERROR_INVALID_FUNCTION: u32 = 1;
pub const ERROR_FILE_NOT_FOUND: u32 = 2;
pub const ERROR_NOT_ENOUGH_MEMORY: u32 = 8;
pub const ERROR_INVALID_PARAMETER: u32 = 87;
pub const ERROR_NOACCESS: u32 = 998;

fn process_slot() -> &'static Mutex<Option<Arc<WispProcess>>> {
    static SLOT: OnceLock<Mutex<Option<Arc<WispProcess>>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

static NEXT_TID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1000);
static CURRENT_IMAGE_BASE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub fn install_process(process: Arc<WispProcess>) {
    *process_slot().lock().expect("Win32 process slot poisoned") = Some(process);
}

pub fn current_process() -> Option<Arc<WispProcess>> {
    process_slot().lock().expect("Win32 process slot poisoned").clone()
}

pub fn set_current_image_base(base: u64) {
    CURRENT_IMAGE_BASE.store(base, std::sync::atomic::Ordering::Release);
}

pub fn current_image_base() -> u64 {
    CURRENT_IMAGE_BASE.load(std::sync::atomic::Ordering::Acquire)
}

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
    if t.is_null() { 0 } else {
        unsafe { std::ptr::read_unaligned(t.add(wisp_core::teb::TEB_LAST_ERROR_OFFSET) as *const u32) }
    }
}

#[inline]
pub fn set_last_error(v: u32) {
    let t = wisp_core::teb::current_teb_base();
    if !t.is_null() {
        unsafe { std::ptr::write_unaligned(t.add(wisp_core::teb::TEB_LAST_ERROR_OFFSET) as *mut u32, v); }
    }
}

pub fn virtual_alloc(address: *mut c_void, size: usize, allocation_type: u32, protection: u32) -> *mut c_void {
    if size == 0 || allocation_type & (MEM_COMMIT | MEM_RESERVE) == 0 {
        set_last_error(ERROR_INVALID_PARAMETER);
        return std::ptr::null_mut();
    }
    let Some(p) = prot(protection) else { set_last_error(ERROR_INVALID_PARAMETER); return std::ptr::null_mut(); };
    let r = unsafe { libc::mmap(address, size, p, libc::MAP_PRIVATE | libc::MAP_ANONYMOUS, -1, 0) };
    if r == libc::MAP_FAILED { set_last_error(ERROR_NOT_ENOUGH_MEMORY); std::ptr::null_mut() } else { set_last_error(ERROR_SUCCESS); r }
}

pub fn virtual_protect(address: *mut c_void, size: usize, protection: u32, old: &mut u32) -> bool {
    if address.is_null() || size == 0 { set_last_error(ERROR_INVALID_PARAMETER); return false; }
    let Some(p) = prot(protection) else { set_last_error(ERROR_INVALID_PARAMETER); return false; };
    if unsafe { libc::mprotect(address, size, p) } != 0 { set_last_error(ERROR_NOACCESS); return false; }
    *old = PAGE_READWRITE;
    set_last_error(ERROR_SUCCESS);
    true
}

pub fn tls_alloc(process: &WispProcess) -> u32 {
    process.tls_alloc().map(|v| v as u32).unwrap_or(TLS_OUT_OF_INDEXES)
}
pub fn tls_free(process: &WispProcess, index: u32) -> bool { process.tls_free(index as usize) }
pub fn tls_get_value(process: &WispProcess, index: u32) -> usize { process.tls_get_value(index as usize).unwrap_or(0) }
pub fn tls_set_value(process: &WispProcess, index: u32, value: usize) -> bool { process.tls_set_value(index as usize, value) }

pub type ThreadStart = usize;

pub fn create_thread(process: &Arc<WispProcess>, start: ThreadStart, parameter: *mut c_void) -> Result<Handle, std::io::Error> {
    let parameter = parameter as usize;
    process.create_thread(move || {
        unsafe { wisp_invoke_thread_start_msabi(start, parameter as *mut c_void); }
    })
}

extern "C" {
    fn wisp_invoke_thread_start_msabi(start: usize, parameter: *mut c_void) -> u32;
    fn wisp_abi_GetLastError() -> u32;
    fn wisp_abi_SetLastError(value: u32);
    fn wisp_abi_VirtualAlloc(address: *mut c_void, size: usize, allocation_type: u32, protection: u32) -> *mut c_void;
    fn wisp_abi_VirtualProtect(address: *mut c_void, size: usize, protection: u32, old: *mut u32) -> i32;
    fn wisp_abi_TlsAlloc() -> u32;
    fn wisp_abi_TlsFree(index: u32) -> i32;
    fn wisp_abi_TlsGetValue(index: u32) -> *mut c_void;
    fn wisp_abi_TlsSetValue(index: u32, value: *mut c_void) -> i32;
    fn wisp_abi_CreateThread(security: *mut c_void, stack_size: usize, start: usize, parameter: *mut c_void, creation_flags: u32, thread_id: *mut u32) -> usize;
    fn wisp_abi_WaitForSingleObject(handle: usize, milliseconds: u32) -> u32;
    fn wisp_abi_CloseHandle(handle: usize) -> i32;
    fn wisp_abi_GetModuleHandleA(name: *const c_char) -> usize;
    fn wisp_abi_GetModuleHandleW(name: *const u16) -> usize;
    fn wisp_abi_GetProcAddress(module: usize, name: *const c_char) -> usize;
    fn wisp_abi_ExitProcess(code: u32);
}

#[unsafe(no_mangle)]
pub extern "C" fn wisp_impl_GetLastError() -> u32 { get_last_error() }
#[unsafe(no_mangle)]
pub extern "C" fn wisp_impl_SetLastError(value: u32) { set_last_error(value); }
#[unsafe(no_mangle)]
pub extern "C" fn wisp_impl_VirtualAlloc(address: *mut c_void, size: usize, allocation_type: u32, protection: u32) -> *mut c_void { virtual_alloc(address, size, allocation_type, protection) }
#[unsafe(no_mangle)]
pub extern "C" fn wisp_impl_VirtualProtect(address: *mut c_void, size: usize, protection: u32, old: *mut u32) -> i32 {
    if old.is_null() { set_last_error(ERROR_INVALID_PARAMETER); return 0; }
    let result = unsafe { virtual_protect(address, size, protection, &mut *old) };
    i32::from(result)
}
#[unsafe(no_mangle)]
pub extern "C" fn wisp_impl_TlsAlloc() -> u32 {
    let Some(process) = current_process() else { set_last_error(ERROR_NOT_ENOUGH_MEMORY); return TLS_OUT_OF_INDEXES; };
    tls_alloc(&process)
}
#[unsafe(no_mangle)]
pub extern "C" fn wisp_impl_TlsFree(index: u32) -> i32 {
    let Some(process) = current_process() else { set_last_error(ERROR_INVALID_FUNCTION); return 0; };
    i32::from(tls_free(&process, index))
}
#[unsafe(no_mangle)]
pub extern "C" fn wisp_impl_TlsGetValue(index: u32) -> *mut c_void {
    let Some(process) = current_process() else { set_last_error(ERROR_INVALID_FUNCTION); return std::ptr::null_mut(); };
    let value = tls_get_value(&process, index);
    set_last_error(ERROR_SUCCESS);
    value as *mut c_void
}
#[unsafe(no_mangle)]
pub extern "C" fn wisp_impl_TlsSetValue(index: u32, value: *mut c_void) -> i32 {
    let Some(process) = current_process() else { set_last_error(ERROR_INVALID_FUNCTION); return 0; };
    let ok = tls_set_value(&process, index, value as usize);
    if !ok { set_last_error(ERROR_INVALID_PARAMETER); }
    i32::from(ok)
}
#[unsafe(no_mangle)]
pub extern "C" fn wisp_impl_CreateThread(start: usize, parameter: *mut c_void, _flags: u32, thread_id: *mut u32) -> usize {
    let Some(process) = current_process() else { set_last_error(ERROR_INVALID_FUNCTION); return 0; };
    match create_thread(&process, start, parameter) {
        Ok(handle) => {
            if !thread_id.is_null() { unsafe { *thread_id = NEXT_TID.fetch_add(1, std::sync::atomic::Ordering::Relaxed); } }
            set_last_error(ERROR_SUCCESS);
            handle.0 as usize
        }
        Err(_) => { set_last_error(ERROR_NOT_ENOUGH_MEMORY); 0 }
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn wisp_impl_WaitForSingleObject(handle: usize, milliseconds: u32) -> u32 {
    let Some(process) = current_process() else { set_last_error(ERROR_INVALID_FUNCTION); return WAIT_FAILED; };
    let h = Handle(handle as u32);
    let start = Instant::now();
    loop {
        match process.thread_state(h) {
            Some(wisp_core::ThreadState::Exited) => {
                let _ = process.wait_thread(h);
                set_last_error(ERROR_SUCCESS);
                return WAIT_OBJECT_0;
            }
            Some(wisp_core::ThreadState::Running) => {
                if milliseconds == 0 { set_last_error(ERROR_SUCCESS); return WAIT_TIMEOUT; }
                if milliseconds != WAIT_INFINITE && start.elapsed() >= Duration::from_millis(milliseconds as u64) {
                    set_last_error(ERROR_SUCCESS);
                    return WAIT_TIMEOUT;
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            None => { set_last_error(ERROR_INVALID_PARAMETER); return WAIT_FAILED; }
        }
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn wisp_impl_CloseHandle(handle: usize) -> i32 {
    let Some(process) = current_process() else { set_last_error(ERROR_INVALID_FUNCTION); return 0; };
    let ok = process.close_thread(Handle(handle as u32));
    if !ok { set_last_error(ERROR_INVALID_PARAMETER); }
    i32::from(ok)
}
#[unsafe(no_mangle)]
pub extern "C" fn wisp_impl_GetModuleHandleA(name: *const c_char) -> usize {
    if name.is_null() { return current_image_base() as usize; }
    let name = unsafe { CStr::from_ptr(name) }.to_string_lossy();
    crate::builtin::module_handle(&name).unwrap_or(0) as usize
}
#[unsafe(no_mangle)]
pub extern "C" fn wisp_impl_GetModuleHandleW(name: *const u16) -> usize {
    if name.is_null() { return current_image_base() as usize; }
    let mut units = Vec::new();
    for i in 0..32768usize {
        let ch = unsafe { *name.add(i) };
        if ch == 0 { break; }
        units.push(ch);
    }
    let text = String::from_utf16_lossy(&units);
    crate::builtin::module_handle(&text).unwrap_or(0) as usize
}
#[unsafe(no_mangle)]
pub extern "C" fn wisp_impl_GetProcAddress(module: usize, name: *const c_char) -> usize {
    if name.is_null() { return 0; }
    let ordinal = name as usize;
    if ordinal <= u16::MAX as usize {
        let sym = format!("#{ordinal}");
        if module == crate::builtin::KERNEL32_HANDLE || module == crate::builtin::NTDLL_HANDLE {
            return crate::builtin::address_handle(module, &sym).unwrap_or(0) as usize;
        }
    }
    let symbol = unsafe { CStr::from_ptr(name) }.to_string_lossy();
    crate::builtin::address_by_handle(module, &symbol).unwrap_or(0) as usize
}
#[unsafe(no_mangle)]
pub extern "C" fn wisp_impl_ExitProcess(code: u32) -> ! { std::process::exit(code as i32) }
