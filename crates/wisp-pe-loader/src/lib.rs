use std::{ops::Range, path::Path};
use thiserror::Error;

const AMD64: u16 = 0x8664;
const PE32_PLUS: u16 = 0x20b;
const IMPORT: usize = 1;
const BASERELOC: usize = 5;
const TLS: usize = 9;

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
}

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

pub fn inspect(path: impl AsRef<Path>) -> Result<PeImage, PeError> { inspect_bytes(&std::fs::read(path)?) }

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
    let size_of_image = u32_(data,opt+56)?;
    let size_of_headers = u32_(data,opt+60)?;
    let dir_count = u32_(data,opt+108)?.min(16) as usize;
    if image_base == 0 || size_of_image == 0 || size_of_headers == 0 || section_alignment == 0 { return Err(PeError::Malformed("invalid image sizing")); }

    let mut dirs = [DataDirectory::default(); 16];
    for i in 0..dir_count { let p=opt+112+i*8; range(data,p,8)?; dirs[i]=DataDirectory{rva:u32_(data,p)?,size:u32_(data,p+4)?}; }

    let sec_base = opt.checked_add(opt_size).ok_or(PeError::Malformed("section offset overflow"))?;
    range(data, sec_base, nsec*40)?;
    let mut sections=Vec::with_capacity(nsec);
    for i in 0..nsec {
        let p=sec_base+i*40; let mut name=[0;8]; name.copy_from_slice(&data[p..p+8]);
        sections.push(Section{name,virtual_size:u32_(data,p+8)?,virtual_address:u32_(data,p+12)?,raw_size:u32_(data,p+16)?,raw_offset:u32_(data,p+20)?,characteristics:u32_(data,p+36)?});
    }
    let mut image=PeImage{entry_rva,image_base,size_of_image,size_of_headers,section_alignment,sections,directories:dirs,imports:Vec::new(),tls:None,reloc_size:dirs[BASERELOC].size};
    if image.rva_to_file_offset(entry_rva).is_none() { return Err(PeError::Malformed("entry point outside sections")); }
    parse_imports(data,&mut image)?;
    parse_tls(data,&mut image)?;
    Ok(image)
}

fn parse_imports(data:&[u8], image:&mut PeImage)->Result<(),PeError>{
    let dir=image.directories[IMPORT]; if dir.rva==0||dir.size==0{return Ok(());}
    let mut off=image.rva_to_file_offset(dir.rva).ok_or(PeError::Malformed("import directory RVA"))?;
    for _ in 0..4096 {
        range(data,off,20)?; let oft=u32_(data,off)?; let name=u32_(data,off+12)?; let ft=u32_(data,off+16)?;
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
