use std::env;
use wisp_pe_loader::{inspect, LoaderState};

fn main() {
    let Some(path) = env::args().nth(1) else {
        eprintln!("usage: wisp <game.exe>");
        std::process::exit(2);
    };

    match inspect(&path) {
        Ok(image) => {
            println!("Wisp PE64");
            println!("  image base : 0x{:x}", image.image_base);
            println!("  image size : 0x{:x}", image.size_of_image);
            println!("  entry RVA  : 0x{:x}", image.entry_rva);
            println!("  sections   : {}", image.sections.len());
            println!("  imports    : {}", image.imports.len());
            println!("  reloc size : 0x{:x}", image.reloc_size);
            println!("  TLS        : {}", image.tls.is_some());
            for dll in &image.imports { println!("    import {}", dll.name); }
            println!("\nLoader plan:");
            for state in [LoaderState::Validate, LoaderState::Map, LoaderState::Relocate,
                          LoaderState::Imports, LoaderState::DependencyInit, LoaderState::CrtInit,
                          LoaderState::Tls, LoaderState::Entry] {
                println!("  -> {state:?}");
            }
        }
        Err(e) => { eprintln!("Wisp: {e}"); std::process::exit(1); }
    }
}
