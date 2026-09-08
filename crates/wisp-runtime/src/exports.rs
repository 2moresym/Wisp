use wisp_pe_loader::{DataDirectory, PeError, PeImage};

const EXPORT_DIR: usize = 0;
const MAX: usize = 1_000_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportTarget { Address { rva: u32, address: u64 }, Forwarder(String) }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportSymbol { pub name: Option<String>, pub ordinal: u32, pub target: ExportTarget }
#[derive(Debug, Clone, Default)]
pub struct ExportTable { pub module_name: Option<String>, pub ordinal_base: u32, pub symbols: Vec<ExportSymbol> }
impl ExportTable {
    pub fn by_name(&self, name: &str) -> Option<&ExportSymbol> { self.symbols.iter().find(|s| s.name.as_deref().is_some_and(|n| n.eq_ignore_ascii_case(name))) }
    pub fn by_ordinal(&self, ordinal: u32) -> Option<&ExportSymbol> { self.symbols.iter().find(|s| s.ordinal == ordinal) }
}
fn bytes(data: &[u8], off: usize, len: usize) -> Result<&[u8], PeError> {
    let end = off.checked_add(len).ok_or(PeError::Malformed("export range overflow"))?;
    data.get(off..end).ok_or(PeError::Malformed("export range outside file"))
}
fn u16_(d:&[u8],o:usize)->Result<u16,PeError>{Ok(u16::from_le_bytes(bytes(d,o,2)?.try_into().unwrap()))}
fn u32_(d:&[u8],o:usize)->Result<u32,PeError>{Ok(u32::from_le_bytes(bytes(d,o,4)?.try_into().unwrap()))}
fn cstr(data:&[u8],off:usize)->Result<String,PeError>{let s=data.get(off..).ok_or(PeError::Malformed("export string RVA"))?;let n=s.iter().position(|b|*b==0).ok_or(PeError::Malformed("unterminated export string"))?;String::from_utf8(s[..n].to_vec()).map_err(|_|PeError::Malformed("invalid export string"))}

pub fn parse_exports(data:&[u8], image:&PeImage)->Result<Option<ExportTable>,PeError>{
    let dir:DataDirectory=image.directories[EXPORT_DIR]; if dir.rva==0||dir.size==0{return Ok(None)}
    let off=image.rva_to_file_offset(dir.rva).ok_or(PeError::Malformed("export directory RVA"))?; bytes(data,off,40)?;
    let name_rva=u32_(data,off+12)?; let ordinal_base=u32_(data,off+16)?; let funcs=u32_(data,off+20)? as usize; let names=u32_(data,off+24)? as usize; let frva=u32_(data,off+28)?; let nrva=u32_(data,off+32)?; let orva=u32_(data,off+36)?;
    if funcs>MAX||names>MAX||names>funcs{return Err(PeError::Malformed("invalid export counts"))}
    let module_name=if name_rva==0{None}else{Some(cstr(data,image.rva_to_file_offset(name_rva).ok_or(PeError::Malformed("export module name RVA"))?)?)};
    let fo=image.rva_to_file_offset(frva).ok_or(PeError::Malformed("export function array RVA"))?; bytes(data,fo,funcs.checked_mul(4).ok_or(PeError::Malformed("export array overflow"))?)?;
    let no=if names==0{None}else{Some(image.rva_to_file_offset(nrva).ok_or(PeError::Malformed("export names RVA"))?)}; let oo=if names==0{None}else{Some(image.rva_to_file_offset(orva).ok_or(PeError::Malformed("export ordinals RVA"))?)};
    let mut name_map=vec![None;funcs];
    if let (Some(no),Some(oo))=(no,oo){bytes(data,no,names.checked_mul(4).ok_or(PeError::Malformed("export name array overflow"))?)?;bytes(data,oo,names.checked_mul(2).ok_or(PeError::Malformed("export ordinal array overflow"))?)?;for i in 0..names{let nr=u32_(data,no+i*4)?;let idx=u16_(data,oo+i*2)? as usize;if idx>=funcs{return Err(PeError::Malformed("export ordinal index"))}let so=image.rva_to_file_offset(nr).ok_or(PeError::Malformed("export name RVA"))?;name_map[idx]=Some(cstr(data,so)?);}}
    let end_rva=dir.rva.checked_add(dir.size).ok_or(PeError::Malformed("export range overflow"))?; let mut symbols=Vec::new();
    for i in 0..funcs{let rva=u32_(data,fo+i*4)?;if rva==0{continue;}let ordinal=ordinal_base.checked_add(i as u32).ok_or(PeError::Malformed("export ordinal overflow"))?;let target=if rva>=dir.rva&&rva<end_rva{ExportTarget::Forwarder(cstr(data,image.rva_to_file_offset(rva).ok_or(PeError::Malformed("export forwarder RVA"))?)?)}else{ExportTarget::Address{rva,address:image.image_base.checked_add(rva as u64).ok_or(PeError::Malformed("export address overflow"))?}};symbols.push(ExportSymbol{name:name_map[i].clone(),ordinal,target});}
    Ok(Some(ExportTable{module_name,ordinal_base,symbols}))
}
