unsafe extern "C" {
    fn wisp_glue_abi_version() -> u32;
}

pub fn abi_version() -> u32 {
    unsafe { wisp_glue_abi_version() }
}
