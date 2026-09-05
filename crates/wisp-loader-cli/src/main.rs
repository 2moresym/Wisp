use std::env;
use wisp_pe_loader::{dependency_paths, inspect, map_image, LoaderState};

fn main() {
    let Some(path) = env::args().nth(1) else {
        eprintln!("usage: wisp <game.exe>");
        std::process::exit(2);
    };

    let image = match inspect(&path) {
        Ok(image) => image,
        Err(e) => { eprintln!("Wisp: {e}"); std::process::exit(1); }
    };

    println!("Wisp PE64");
    println!("  image base : 0x{:x}", image.image_base);
    println!("  image size : 0x{:x}", image.size_of_image);
    println!("  entry RVA  : 0x{:x}", image.entry_rva);
    println!("  sections   : {}", image.sections.len());
    println!("  imports    : {}", image.imports.len());
    println!("  reloc size : 0x{:x}", image.reloc_size);
    println!("  TLS        : {}", image.tls.is_some());
    for section in &image.sections {
        println!("    section {:<8} RVA 0x{:x} size 0x{:x}",
            wisp_pe_loader::PeImage::section_name(section), section.virtual_address,
            section.virtual_size.max(section.raw_size));
    }
    for dll in &image.imports { println!("    import {}", dll.name); }

    println!("\nLoader plan:");
    for state in [LoaderState::Validate, LoaderState::Map, LoaderState::Relocate,
                  LoaderState::Imports, LoaderState::DependencyInit, LoaderState::CrtInit,
                  LoaderState::Tls, LoaderState::Entry] {
        println!("  -> {state:?}");
    }

    if !image.imports.is_empty() {
        println!("\nDependencies:");
        for (name, found) in dependency_paths(&path, &image) {
            match found {
                Some(p) => println!("  {} -> {}", name, p.display()),
                None => println!("  {} -> <not found>", name),
            }
        }
        println!("\nWisp: imported PE execution is not enabled yet; dependency metadata is ready.");
        return;
    }

    println!("\nMapping image...");
    match map_image(&path) {
        Ok(mapped) => {
            println!("  mapped base : 0x{:x}", mapped.base());
            println!("  entry       : 0x{:x}", mapped.entry());
            println!("  relocated   : {}", mapped.relocated);
            println!("\nExecuting entry point...");
            let rc = unsafe { mapped.call_entry() };
            println!("  entry return: {}", rc);
        }
        Err(e) => { eprintln!("Wisp map: {e}"); std::process::exit(1); }
    }
}
