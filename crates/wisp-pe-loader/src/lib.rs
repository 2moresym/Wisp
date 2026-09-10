use std::{
    fs,
    ops::Range,
    path::{Path, PathBuf},
};
use thiserror::Error;

const AMD64: u16 = 0x8664;
const PE32_PLUS: u16 = 0x20b;
const EXPORT: usize = 0;
const IMPORT: usize = 1;
const BASERELOC: usize = 5;
const TLS: usize = 9;
const IMAGE_REL_BASED_ABSOLUTE: u16 = 0;
const IMAGE_REL_BASED_DIR64: u16 = 10;
const PAGE_SIZE: usize = 4096;
const MAP_FIXED_NOREPLACE: i32 = 0x100000;

#[derive(Debug, Error)]
pub enum PeError {
    #[error("not an x86-64 PE image")]
    InvalidImage,
    #[error("unsupported PE feature: {0}")]
    Unsupported(&'static str),
    #[error("malformed PE: {0}")]
    Malformed(&'static str),
    #[error("I/O: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoaderPhase {
    Validate,
    Map,
    Relocate,
    ResolveImports,
    InitializeDependencies,
    InitializeMain,
    InitializeCrt,
    InitializeTls,
    Enter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DataDirectory {
    pub rva: u32,
    pub size: u32,
}

#[derive(Debug, Clone)]
pub struct Section {
    pub name: [u8; 8],
    pub virtual_address: u32,
    pub virtual_size: u32,
    pub raw_offset: u32,
    pub raw_size: u32,
    pub characteristics: u32,
}

#[derive(Debug, Clone)]
pub struct ImportDll {
    pub name: String,
    pub thunk_rva: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct TlsInfo {
    pub raw_start: u64,
    pub raw_end: u64,
    pub callbacks_va: u64,
}

#[derive(Debug, Clone)]
pub struct PeImage {
    pub entry_rva: u32,
    pub image_base: u64,
    pub size_of_image: u32,
    pub size_of_headers: u32,
    pub section_alignment: u32,
    pub sections: Vec<Section>,
    pub directories: [DataDirectory; 16],
    pub imports: Vec<ImportDll>,
    pub tls: Option<TlsInfo>,
    pub reloc_size: u32,
}

impl PeImage {
    pub fn rva_to_file_offset(&self, rva: u32) -> Option<usize> {
        if rva < self.size_of_headers {
            return Some(rva as usize);
        }
        self.sections.iter().find_map(|s| {
            let size = s.virtual_size.max(s.raw_size);
            let delta = rva.checked_sub(s.virtual_address)?;
            (delta < size).then_some(s.raw_offset as usize + delta as usize)
        })
    }

    pub fn image_range(&self) -> Range<u64> {
        self.image_base..self.image_base + self.size_of_image as u64
    }

    pub fn section_name(section: &Section) -> String {
        let end = section.name.iter().position(|&b| b == 0).unwrap_or(8);
        String::from_utf8_lossy(&section.name[..end]).into_owned()
    }
}

#[derive(Debug)]
pub struct MappedImage {
    ptr: *mut u8,
    len: usize,
    pub base: u64,
    pub entry: u64,
    pub relocated: bool,
}

impl MappedImage {
    pub fn entry(&self) -> u64 {
        self.entry
    }
    pub fn base(&self) -> u64 {
        self.base
    }
    pub fn as_ptr(&self) -> *mut u8 {
        self.ptr
    }

    /// Execute the PE entry point as a no-argument x86-64 function.
    /// Uses Windows x64 (ms_abi) calling convention.
    pub unsafe fn call_entry(&self) -> i32 {
        let f: extern "win64" fn() -> i32 = unsafe { std::mem::transmute(self.entry as usize) };
        f()
    }
}

impl Drop for MappedImage {
    fn drop(&mut self) {
        if !self.ptr.is_null() && self.len != 0 {
            unsafe {
                libc::munmap(self.ptr.cast(), self.len);
            }
        }
    }
}

unsafe impl Send for MappedImage {}
unsafe impl Sync for MappedImage {}

fn range(data: &[u8], off: usize, len: usize) -> Result<Range<usize>, PeError> {
    let end = off
        .checked_add(len)
        .ok_or(PeError::Malformed("offset overflow"))?;
    if end > data.len() {
        Err(PeError::Malformed("truncated image"))
    } else {
        Ok(off..end)
    }
}

fn u16_(d: &[u8], o: usize) -> Result<u16, PeError> {
    Ok(u16::from_le_bytes(d[range(d, o, 2)?].try_into().unwrap()))
}

fn u32_(d: &[u8], o: usize) -> Result<u32, PeError> {
    Ok(u32::from_le_bytes(d[range(d, o, 4)?].try_into().unwrap()))
}

fn u64_(d: &[u8], o: usize) -> Result<u64, PeError> {
    Ok(u64::from_le_bytes(d[range(d, o, 8)?].try_into().unwrap()))
}

fn cstr(d: &[u8], o: usize) -> Result<String, PeError> {
    let s = d.get(o..).ok_or(PeError::Malformed("string offset"))?;
    let n = s
        .iter()
        .position(|&b| b == 0)
        .ok_or(PeError::Malformed("unterminated string"))?;
    String::from_utf8(s[..n].to_vec()).map_err(|_| PeError::Malformed