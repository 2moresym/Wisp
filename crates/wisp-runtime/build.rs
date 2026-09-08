fn main() {
    cc::Build::new()
        .file("src/win32_abi.c")
        .flag_if_supported("-O3")
        .compile("wisp_win32_abi");
}
