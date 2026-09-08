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

#[inline]
pub fn address(module: &str, symbol: &str) -> Option<u64> {
    module_handle(module).and_then(|_| win32::abi_address(symbol))
}

#[inline]
pub fn address_by_handle(module: u64, symbol: &str) -> Option<u64> {
    match module {
        NTDLL_HANDLE | KERNEL32_HANDLE => win32::abi_address(symbol),
        _ => None,
    }
}
