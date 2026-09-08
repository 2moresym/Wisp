use wisp_pe_loader::{DataDirectory, PeError, PeImage};

const IMPORT_DIR: usize = 1;
const ORDINAL_FLAG: u64 = 1u64 << 63;
const MAX_DESCRIPTORS: usize = 4096;
const MAX_THUNKS: usize = 1_000_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportSymbol { Name { hint: u16, name: String }, Ordinal(u16) }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportBinding { pub dll: String, pub lookup_rva: u32, pub iat_rva: u32, pub symbol: ImportSymbol }

fn bytes(data:&[u8],off:usize,len:usize)->Result<&[u8],PeError>{let end=off.checked_add(len).ok_or(PeError::Malformed("import range overflow"))?;data.get(off..end).ok_or(PeError::Malformed("import range outside file"))}
fn u16_(d:&[u8],o:usize)->Result<u16,PeError>{Ok(u16::from_le_bytes(bytes(d,o,2)?.try_into().unwrap()))}
fn u32_(d:&[u8],o:usize)->Result<u32,PeError>{Ok(u32::from_le_bytes(bytes(d,o,4)?.try_into().unwrap()))}
fn u64_(d:&[u8],o:usize)->Result<u64,PeError>{Ok(u64::from_le_bytes(bytes(d,o,8)?.try_into().unwrap()))}
fn cstr(d:&[u8],o:usize)->Result<String,PeError>{let s=d.get(o..).ok_or(PeError::Malformed("import string RVA"))?;let n=s.iter().position(|b|*b==0).ok_or(PeError::Malformed("unterminated import string"))?;String::from_utf8(s[..n].to_vec()).map_err(|_|PeError::Malformed("invalid import string"))}

pub fn parse_import_bindings(data:&[u8],image:&PeImage)->Result<Vec<ImportBinding>,PeError>{
 let dir:DataDirectory=image.directories[IMPORT_DIR];if dir.rva==0||dir.size==0{return Ok(Vec::new())}
 let mut off=image.rva_to_file_offset(dir.rva).ok_or(PeError::Malformed("import directory RVA"))?;let end=off.checked_add(dir.size as usize).ok_or(PeError::Malformed("import directory overflow"))?;if end>data.len(){return Err(PeError::Malformed("import directory exceeds file"))}
 let mut out=Vec::new();
 for _ in 0..MAX_DESCRIPTORS{bytes(data,off,20)?;let oft=u32_(data,off)?;let name_rva=u32_(data,off+12)?;let ft=u32_(data,off+16)?;off+=20;if oft==0&&name_rva==0&&ft==0{return Ok(out)}if name_rva==0||ft==0{return Err(PeError::Malformed("invalid import descriptor"))}
  let name_off=image.rva_to_file_offset(name_rva).ok_or(PeError::Malformed("import DLL name RVA"))?;let dll=cstr(data,name_off)?;let lookup_rva=if oft!=0{oft}else{ft};
  for i in 0..MAX_THUNKS{let delta=(i as u32).checked_mul(8).ok_or(PeError::Malformed("import thunk overflow"))?;let lookup=lookup_rva.checked_add(delta).ok_or(PeError::Malformed("lookup RVA overflow"))?;let iat=ft.checked_add(delta).ok_or(PeError::Malformed("IAT RVA overflow"))?;let lo=image.rva_to_file_offset(lookup).ok_or(PeError::Malformed("lookup RVA"))?;let value=u64_(data,lo)?;if value==0{break}let symbol=if value&ORDINAL_FLAG!=0{ImportSymbol::Ordinal((value&0xffff) as u16)}else{let nr=u32::try_from(value).map_err(|_|PeError::Malformed("import name RVA too large"))?;let no=image.rva_to_file_offset(nr).ok_or(PeError::Malformed("import symbol RVA"))?;ImportSymbol::Name{hint:u16_(data,no)?,name:cstr(data,no.checked_add(2).ok_or(PeError::Malformed("import name overflow"))?)?}};out.push(ImportBinding{dll:dll.clone(),lookup_rva:lookup,iat_rva:iat,symbol});}
 }
 Err(PeError::Malformed("import directory too large or unterminated"))
}
