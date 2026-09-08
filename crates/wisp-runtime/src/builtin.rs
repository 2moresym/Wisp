use crate::win32;

pub const NTDLL_HANDLE: u64 = 0xffff_ffff_ffff_0001;
pub const KERNEL32_HANDLE: u64 = 0xffff_ffff_ffff_0002;

#[inline]
pub fn module_handle(name: &str) -> Option<u64> {
    match name.to_ascii_lowercase().as_str() {
        "ntdll.dll" | "ntdll" => Some(NTDLL_HANDLE),
        "kernel32.dll" | "kernel32" | "kernelbase.dll" | "kernelbase" => Some(KERNEL32_HANDLE),
        _ => None,
    }
}

#[inline]
pub fn is_builtin(name: &str) -> bool { module_handle(name).is_some() }

pub fn address(module: &str, symbol: &str) -> Option<u64> {
    if !is_builtin(module) { return None; }
    macro_rules! addr {
        ($name:literal, $fn_name:ident) => {
            if symbol.eq_ignore_ascii_case($name) { return Some(win32::$fn_name as usize as u64); }
        };
    }
    addr!("GetLastError", wisp_abi_GetLastError);
    addr!("SetLastError", wisp_abi_SetLastError);
    addr!("VirtualAlloc", wisp_abi_VirtualAlloc);
    addr!("VirtualProtect", wisp_abi_VirtualProtect);
    addr!("TlsAlloc", wisp_abi_TlsAlloc);
    addr!("TlsFree", wisp_abi_TlsFree);
    addr!("TlsGetValue", wisp_abi_TlsGetValue);
    addr!("TlsSetValue", wisp_abi_TlsSetValue);
    addr!("CreateThread", wisp_abi_CreateThread);
    addr!("WaitForSingleObject", wisp_abi_WaitForSingleObject);
    addr!("CloseHandle", wisp_abi_CloseHandle);
    addr!("GetModuleHandleA", wisp_abi_GetModuleHandleA);
    addr!("GetModuleHandleW", wisp_abi_GetModuleHandleW);
    addr!("GetProcAddress", wisp_abi_GetProcAddress);
    addr!("ExitProcess", wisp_abi_ExitProcess);
    None
}
