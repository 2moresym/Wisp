fn main() {
    cc::Build::new().file("src/wisp_glue.c").flag_if_supported("-O3").compile("wisp_glue");
}
