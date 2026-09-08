//! Minimal x64 Windows-compatible Process Environment Block anchor.
//!
//! The public PEB definition is intentionally opaque. Wisp keeps an
//! explicitly sized byte-backed representation so fields can be added with
//! verified offsets without coupling the runtime to a particular Windows
//! SDK's reserved-field layout.

use std::{mem::size_of, ptr};

const PEB_SIZE: usize = 0x1000;
const IMAGE_BASE_OFFSET: usize = 0x10;
const LDR_OFFSET: usize = 0x18;
const PROCESS_PARAMETERS_OFFSET: usize = 0x20;

#[repr(align(16))]
struct PebBytes([u8; PEB_SIZE]);

pub struct Peb {
    bytes: Box<PebBytes>,
}

impl Peb {
    pub fn new() -> Self {
        Self { bytes: Box::new(PebBytes([0; PEB_SIZE])) }
    }

    #[inline]
    pub fn as_ptr(&self) -> *mut u8 {
        self.bytes.0.as_ptr() as *mut u8
    }

    #[inline]
    pub fn image_base(&self) -> *mut u8 {
        self.read_ptr(IMAGE_BASE_OFFSET)
    }

    #[inline]
    pub fn set_image_base(&mut self, value: *mut u8) {
        self.write_ptr(IMAGE_BASE_OFFSET, value);
    }

    #[inline]
    pub fn ldr(&self) -> *mut u8 {
        self.read_ptr(LDR_OFFSET)
    }

    #[inline]
    pub fn set_ldr(&mut self, value: *mut u8) {
        self.write_ptr(LDR_OFFSET, value);
    }

    #[inline]
    pub fn process_parameters(&self) -> *mut u8 {
        self.read_ptr(PROCESS_PARAMETERS_OFFSET)
    }

    #[inline]
    pub fn set_process_parameters(&mut self, value: *mut u8) {
        self.write_ptr(PROCESS_PARAMETERS_OFFSET, value);
    }

    #[inline]
    fn read_ptr(&self, offset: usize) -> *mut u8 {
        debug_assert!(offset + size_of::<usize>() <= PEB_SIZE);
        unsafe { ptr::read_unaligned(self.as_ptr().add(offset) as *const usize) as *mut u8 }
    }

    #[inline]
    fn write_ptr(&mut self, offset: usize, value: *mut u8) {
        debug_assert!(offset + size_of::<usize>() <= PEB_SIZE);
        unsafe {
            ptr::write_unaligned(self.as_ptr().add(offset) as *mut usize, value as usize);
        }
    }
}

impl Default for Peb {
    fn default() -> Self { Self::new() }
}
