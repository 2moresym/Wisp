use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PeError {
    #[error("not an x86-64 PE image")]
    InvalidImage,
    #[error("unsupported PE feature")]
    Unsupported,
    #[error("I/O: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoaderState { Validate, Map, Relocate, Imports, DependencyInit, CrtInit, Tls, Entry, Running, Exited }

pub struct PeImage { pub entry_rva: u32, pub image_base: u64, pub size_of_image: u32 }

/// Parses enough PE metadata to establish a safe loading plan. Mapping/execution remains explicit.
pub fn inspect(path: impl AsRef<Path>) -> Result<PeImage, PeError> {
    let data = std::fs::read(path)?;
    if data.len() < 0x40 || &data[0..2] != b"MZ" { return Err(PeError::InvalidImage); }
    let pe = u32::from_le_bytes(data[0x3c..0x40].try_into().unwrap()) as usize;
    if pe.checked_add(24).map_or(true, |x| x > data.len()) || &data[pe..pe+4] != b"PE\0\0" { return Err(PeError::InvalidImage); }
    let machine = u16::from_le_bytes(data[pe+4..pe+6].try_into().unwrap());
    let optional_size = u16::from_le_bytes(data[pe+20..pe+22].try_into().unwrap()) as usize;
    if machine != 0x8664 || pe + 24 + optional_size > data.len() { return Err(PeError::Unsupported); }
    let opt = pe + 24;
    if u16::from_le_bytes(data[opt..opt+2].try_into().unwrap()) != 0x20b { return Err(PeError::Unsupported); }
    Ok(PeImage {
        entry_rva: u32::from_le_bytes(data[opt+16..opt+20].try_into().unwrap()),
        image_base: u64::from_le_bytes(data[opt+24..opt+32].try_into().unwrap()),
        size_of_image: u32::from_le_bytes(data[opt+56..opt+60].try_into().unwrap()),
    })
}
