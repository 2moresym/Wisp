use std::{fs, ops::Range, path::{Path, PathBuf}};
use thiserror::Error;

const AMD64: u16 = 0x8664;
const PE32_PLUS: u16 = 0x20b;
const IMPORT: usize = 1;
const BASERELOC: usize = 5;
const TLS: usize = 9;
const EXPORT: usize = 0;
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
pub enum LoaderState { Validate, Map, Relocate, Imports, DependencyInit, CrtInit, Tls, Entry, Running, Exited }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DataDirectory { pub rva: u32, pub size: u32 }

#[derive(Debug, Clone)]
pub struct Section {
    pub name: [u8; 8], pub virtual_address: u32, pub virtual_size: u32,
    pub raw_offset: u32, pub raw_size: u32, pub characteristics: u32,
}

#[derive(Debug, Clone)]
pub struct ImportDll { pub name: String, pub thunk_rva: u32 }

#[derive(Debug, Clone, Copy)]
pub struct TlsInfo { pub raw_start: u64, pub raw_end: u64, pub callbacks_va: u64 }

#[derive(Debug, Clone)]
pub struct PeImage {
    pub entry_rva: u32, pub image_base: u64, pub size_of_image: u32,
    pub size_of_headers: u32, pub section_alignment: u32,
    pub sections: Vec<Section>, pub directories: [DataDirectory; 16],
    pub imports: Vec<ImportDll>, pub tls: Option<TlsInfo>, pub reloc_size: u32,
}

impl PeImage {
    pub fn rva_to_file_offset(&self, rva: u32) -> Option<usize> {
        if rva < self.size_of_headers { return Some(rva as usize); }
        self.sections.iter().find_map(|s| {
            let size = s.virtual_size.max(s.raw_size);
            let delta = rva.checked_sub(s.virtual_address)?;
            (delta < size).then_some(s.raw_offset as usize + delta as usize)
        })
    }

    pub fn image_range(&self) -> Range<u64> { self.image_base..self.image_base + self.size_of_image as u64 }

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
    pub fn entry(&self) -> u64 { self.entry }
    pub fn base(&self) -> u64 { self.base }
    pub fn as_ptr(&self) -> *mut u8 { self.ptr }

    /// Execute the PE entry point as a no-argument x86-64 function.
    /// Only suitable for the dependency-free test fixture until the Windows
    /// process/thread environment exists.
    pub unsafe fn call_entry(&self) -> i32 {
        let f: extern "C" fn() -> i32 = unsafe { std::mem::transmute(self.entry as usize) };
        f()
    }
}

impl Drop for MappedImage {
    fn drop(&mut self) {
        if !self.ptr.is_null() && self.len != 0 {
            unsafe { libc::munmap(self.ptr.cast(), self.len); }
        }
    }
}

unsafe impl Send for MappedImage {}
unsafe impl Sync for MappedImage {}

fn range(data: &[u8], off: usize, len: usize) -> Result<Range<usize>, PeError> {
    let end = off.checked_add(len).ok_or(PeError::Malformed("offset overflow"))?;
    if end > data.len() { Err(PeError::Malformed("truncated image")) } else { Ok(off..end) }
}
fn u16_(d: &[u8], o: usize) -> Result<u16, PeError> { Ok(u16::from_le_bytes(d[range(d,o,2)?].try_into().unwrap())) }
fn u32_(d: &[u8], o: usize) -> Result<u32, PeError> { Ok(u32::from_le_bytes(d[range(d,o,4)?].try_into().unwrap())) }
fn u64_(d: &[u8], o: usize) -> Result<u64, PeError> { Ok(u64::from_le_bytes(d[range(d,o,8)?].try_into().unwrap())) }
fn cstr(d: &[u8], o: usize) -> Result<String, PeError> {
    let s = d.get(o..).ok_or(PeError::Malformed("string offset"))?;
    let n = s.iter().position(|&b| b == 0).ok_or(PeError::Malformed("unterminated string"))?;
    String::from_utf8(s[..n].to_vec()).map_err(|_| PeError::Malformed("invalid import name"))
}
fn align_up(v: usize, a: usize) -> Option<usize> {
    let mask = a.checked_sub(1)?;
    v.checked_add(mask).map(|x| x & !mask)
}

pub fn inspect(path: impl AsRef<Path>) -> Result<PeImage, PeError> { inspect_bytes(&fs::read(path)?) }

/// Strict metadata pass. It never maps or executes the image.
pub fn inspect_bytes(data: &[u8]) -> Result<PeImage, PeError> {
    range(data, 0, 0x40)?;
    if &data[..2] != b"MZ" { return Err(PeError::InvalidImage); }
    let pe = u32_(data, 0x3c)? as usize;
    range(data, pe, 24)?;
    if &data[pe..pe+4] != b"PE\0\0" { return Err(PeError::InvalidImage); }
    let machine = u16_(data, pe+4)?;
    let nsec = u16_(data, pe+6)? as usize;
    let opt_size = u16_(data, pe+20)? as usize;
    if machine != AMD64 { return Err(PeError::Unsupported("AMD64 only")); }
    if nsec == 0 || nsec > 96 { return Err(PeError::Malformed("invalid section count")); }
    if opt_size < 112 { return Err(PeError::Malformed("optional header too small")); }
    let opt = pe + 24;
    range(data, opt, opt_size)?;
    if u16_(data,opt)? != PE32_PLUS { return Err(PeError::Unsupported("PE32+ only")); }
    let entry_rva = u32_(data,opt+16)?;
    let image_base = u64_(data,opt+24)?;
    let section_alignment = u32_(data,opt+32)?;
    let file_alignment = u32_(data,opt+36)?;
    let size_of_image = u32_(data,opt+56)?;
    let size_of_headers = u32_(data,opt+60)?;
    let dir_count = u32_(data,opt+108)?.min(16) as usize;
    if image_base == 0 || size_of_image == 0 || size_of_headers == 0 || section_alignment == 0 { return Err(PeError::Malformed("invalid image sizing")); }
    if section_alignment < PAGE_SIZE as u32 && section_alignment < file_alignment { return Err(PeError::Malformed("invalid alignment")); }

    let mut dirs = [DataDirectory::default(); 16];
    for i in 0..dir_count { let p=opt+112+i*8; range(data,p,8)?; dirs[i]=DataDirectory{rva:u32_(data,p)?,size:u32_(data,p+4)?}; }

    let sec_base = opt.checked_add(opt_size).ok_or(PeError::Malformed("section offset overflow"))?;
    range(data, sec_base, nsec*40)?;
    let mut sections=Vec::with_capacity(nsec);
    for i in 0..nsec {
        let p=sec_base+i*40; let mut name=[0;8]; name.copy_from_slice(&data[p..p+8]);
        let virtual_size=u32_(data,p+8)?; let virtual_address=u32_(data,p+12)?;
        let raw_size=u32_(data,p+16)?; let raw_offset=u32_(data,p+20)?; let characteristics=u32_(data,p+36)?;
        if raw_size != 0 { let end=raw_offset.checked_add(raw_size).ok_or(PeError::Malformed("section file range overflow"))?; if end as usize > data.len(){return Err(PeError::Malformed("section exceeds file"));} }
        let span=virtual_size.max(raw_size);
        if span != 0 { let end=virtual_address.checked_add(span).ok_or(PeError::Malformed("section RVA overflow"))?; if end > size_of_image{return Err(PeError::Malformed("section exceeds image"));} }
        sections.push(Section{name,virtual_size,virtual_address,raw_size,raw_offset,characteristics});
    }
    let mut image=PeImage{entry_rva,image_base,size_of_image,size_of_headers,section_alignment,sections,directories:dirs,imports:Vec::new(),tls:None,reloc_size:dirs[BASERELOC].size};
    if image.rva_to_file_offset(entry_rva).is_none() { return Err(PeError::Malformed("entry point outside sections")); }
    parse_imports(data,&mut image)?;
    parse_tls(data,&mut image)?;
    Ok(image)
}

/// Map a validated PE image into the current Linux process. The preferred
/// Windows image base is attempted first; relocations are applied when the
/// kernel gives us a different address.
pub fn map_image(path: impl AsRef<Path>) -> Result<MappedImage, PeError> {
    let path = path.as_ref();
    let data = fs::read(path)?;
    let image = inspect_bytes(&data)?;
    let len = align_up(image.size_of_image as usize, PAGE_SIZE).ok_or(PeError::Malformed("image size overflow"))?;

    let preferred = image.image_base as usize;
    let mut ptr = unsafe {
        libc::mmap(preferred as *mut libc::c_void, len, libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | MAP_FIXED_NOREPLACE, -1, 0)
    };
    if ptr == libc::MAP_FAILED {
        ptr = unsafe { libc::mmap(std::ptr::null_mut(), len, libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_PRIVATE | libc::MAP_ANONYMOUS, -1, 0) };
    }
    if ptr == libc::MAP_FAILED { return Err(PeError::Io(std::io::Error::last_os_error())); }

    let mapped_base = ptr as u64;
    let result = (|| {
        let dst = ptr.cast::<u8>();
        let header_len = (image.size_of_headers as usize).min(data.len());
        unsafe { std::ptr::copy_nonoverlapping(data.as_ptr(), dst, header_len); }
        for s in &image.sections {
            if s.raw_size == 0 { continue; }
            let src = data.get(s.raw_offset as usize..s.raw_offset as usize + s.raw_size as usize)
                .ok_or(PeError::Malformed("section raw range"))?;
            let target = unsafe { dst.add(s.virtual_address as usize) };
            unsafe { std::ptr::copy_nonoverlapping(src.as_ptr(), target, src.len()); }
        }

        let delta = mapped_base as i128 - image.image_base as i128;
        if delta != 0 {
            if image.reloc_size == 0 { return Err(PeError::Unsupported("image relocated but has no base relocations")); }
            apply_relocations(dst, &image, delta)?;
        }
        protect_sections(dst, &image)?;
        let entry = mapped_base.checked_add(image.entry_rva as u64).ok_or(PeError::Malformed("entry address overflow"))?;
        Ok::<_,PeError>((entry, delta != 0))
    })();

    match result {
        Ok((entry, relocated)) => Ok(MappedImage { ptr: ptr.cast(), len, base: mapped_base, entry, relocated }),
        Err(e) => { unsafe { libc::munmap(ptr, len); } Err(e) }
    }
}

fn apply_relocations(base: *mut u8, image: &PeImage, delta: i128) -> Result<(), PeError> {
    let dir = image.directories[BASERELOC];
    let start = image.rva_to_file_offset(dir.rva).ok_or(PeError::Malformed("base relocation RVA"))?;
    let end = start.checked_add(dir.size as usize).ok_or(PeError::Malformed("relocation range overflow"))?;
    // Relocation blocks are read from the original file, so this routine is
    // only called by map_image after the metadata pass. Re-read the file is
    // intentionally avoided; the mapped image contains the relocation bytes.
    let mut cursor = dir.rva as usize;
    let limit = cursor.checked_add(dir.size as usize).ok_or(PeError::Malformed("relocation RVA overflow"))?;
    while cursor < limit {
        if cursor + 8 > image.size_of_image as usize { return Err(PeError::Malformed("relocation block outside image")); }
        let block = unsafe { std::slice::from_raw_parts(base.add(cursor), 8) };
        let page_rva = u32::from_le_bytes(block[0..4].try_into().unwrap());
        let block_size = u32::from_le_bytes(block[4..8].try_into().unwrap()) as usize;
        if block_size < 8 || cursor + block_size > limit { return Err(PeError::Malformed("invalid relocation block")); }
        let count = (block_size - 8) / 2;
        for i in 0..count {
            let raw = unsafe { u16::from_le_bytes(std::slice::from_raw_parts(base.add(cursor+8+i*2),2).try_into().unwrap()) };
            let kind = raw >> 12;
            let off = (raw & 0x0fff) as usize;
            if kind == IMAGE_REL_BASED_ABSOLUTE { continue; }
            if kind != IMAGE_REL_BASED_DIR64 { return Err(PeError::Unsupported("non-DIR64 relocation")); }
            let target_rva = (page_rva as usize).checked_add(off).ok_or(PeError::Malformed("relocation target overflow"))?;
            if target_rva + 8 > image.size_of_image as usize { return Err(PeError::Malformed("relocation target outside image")); }
            let target = unsafe { base.add(target_rva).cast::<u64>() };
            let old = unsafe { std::ptr::read_unaligned(target) };
            let new = (old as i128).checked_add(delta).ok_or(PeError::Malformed("relocation value overflow"))?;
            if !(0..=u64::MAX as i128).contains(&new) { return Err(PeError::Malformed("relocation value out of range")); }
            unsafe { std::ptr::write_unaligned(target, new as u64); }
        }
        cursor += block_size;
    }
    let _ = (start, end);
    Ok(())
}

fn protect_sections(base: *mut u8, image: &PeImage) -> Result<(), PeError> {
    for s in &image.sections {
        let span = s.virtual_size.max(s.raw_size) as usize;
        if span == 0 { continue; }
        let start = (s.virtual_address as usize) & !(PAGE_SIZE - 1);
        let end = align_up(s.virtual_address as usize + span, PAGE_SIZE).ok_or(PeError::Malformed("section permission range overflow"))?;
        let mut prot = 0;
        if s.characteristics & 0x40000000 != 0 { prot |= libc::PROT_READ; }
        if s.characteristics & 0x80000000 != 0 { prot |= libc::PROT_WRITE; }
        if s.characteristics & 0x20000000 != 0 { prot |= libc::PROT_EXEC; }
        if prot == 0 { prot = libc::PROT_NONE; }
        let rc = unsafe { libc::mprotect(base.add(start).cast(), end-start, prot) };
        if rc != 0 { return Err(PeError::Io(std::io::Error::last_os_error())); }
    }
    Ok(())
}

fn parse_imports(data:&[u8], image:&mut PeImage)->Result<(),PeError>{
    let dir=image.directories[IMPORT]; if dir.rva==0||dir.size==0{return Ok(());}
    let mut off=image.rva_to_file_offset(dir.rva).ok_or(PeError::Malformed("import directory RVA"))?;
    let end=off.checked_add(dir.size as usize).ok_or(PeError::Malformed("import directory overflow"))?;
    if end > data.len(){return Err(PeError::Malformed("import directory exceeds file"));}
    for _ in 0..4096 {
        if off + 20 > end { return Err(PeError::Malformed("unterminated import directory")); }
        let oft=u32_(data,off)?; let name=u32_(data,off+12)?; let ft=u32_(data,off+16)?;
        if oft==0&&name==0&&ft==0{return Ok(());}
        let noff=image.rva_to_file_offset(name).ok_or(PeError::Malformed("import name RVA"))?;
        image.imports.push(ImportDll{name:cstr(data,noff)?,thunk_rva:if oft!=0{oft}else{ft}}); off+=20;
    }
    Err(PeError::Malformed("import directory too large or unterminated"))
}

fn parse_tls(data:&[u8], image:&mut PeImage)->Result<(),PeError>{
    let dir=image.directories[TLS]; if dir.rva==0||dir.size==0{return Ok(());}
    let off=image.rva_to_file_offset(dir.rva).ok_or(PeError::Malformed("TLS directory RVA"))?; range(data,off,40)?;
    image.tls=Some(TlsInfo{raw_start:u64_(data,off)?,raw_end:u64_(data,off+8)?,callbacks_va:u64_(data,off+24)?}); Ok(())
}

/// Resolve dependency filenames using a small Windows-like search set. This
/// deliberately does not search the whole host filesystem.
pub fn dependency_paths(image_path: impl AsRef<Path>, image: &PeImage) -> Vec<(String, Option<PathBuf>)> {
    let image_path = image_path.as_ref();
    let mut roots = Vec::new();
    if let Some(parent) = image_path.parent() { roots.push(parent.to_path_buf()); }
    if let Some(windir) = std::env::var_os("WINDIR") { roots.push(PathBuf::from(windir).join("System32")); }
    image.imports.iter().map(|dll| {
        let mut found = None;
        for root in &roots {
            let p = root.join(&dll.name);
            if p.is_file() { found = Some(p); break; }
            let lower = dll.name.to_ascii_lowercase();
            if let Ok(entries) = fs::read_dir(root) {
                for e in entries.flatten() {
                    if e.file_name().to_string_lossy().to_ascii_lowercase() == lower { found=Some(e.path()); break; }
                }
            }
            if found.is_some() { break; }
        }
        (dll.name.clone(), found)
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn align_is_safe() {
        assert_eq!(align_up(1, 4096), Some(4096));
        assert_eq!(align_up(4096, 4096), Some(4096));
    }

    #[test]
    fn malformed_input_is_rejected() {
        assert!(matches!(inspect_bytes(b"MZ"), Err(PeError::Malformed(_))));
    }
}
