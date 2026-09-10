use std::env;

use wisp_pe_loader::{inspect, map_image, LoaderState};
use wisp_runtime::RuntimeLoader;

fn usage() -> ! {
    eprintln!("usage: wisp <game.exe> [--inspect|--load]");
    std::process::exit(2);
}

fn print_image(path: &str) -> wisp_pe_loader::PeImage {
    let image = match inspect(path) {
        Ok(image) => image,
        Err(e) => {
            eprintln!("Wisp: {e}");
            std::process::exit(1);
        }
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
        println!(
            "    section {:<8} RVA 0x{:x} size 0x{:x}",
            wisp_pe_loader::PeImage::section_name(section),
            section.virtual_address,
            section.virtual_size.max(section.raw_size)
        );
    }
    for dll in &image.imports {
        println!("    import {}", dll.name);
    }
    println!("\nLoader plan:");
    for state in [
        LoaderState::Validate,
        LoaderState::Map,
        LoaderState::Relocate,
        LoaderState::Imports,
        LoaderState::DependencyInit,
        LoaderState::CrtInit,
        LoaderState::Tls,
        LoaderState::Entry,
    ] {
        println!("  -> {state:?}");
    }
    image
}

fn main() {
    let mut args = env::args().skip(1);
    let Some(path) = args.next() else { usage(); };
    let mode = args.next().as_deref().unwrap_or("--load");
    if args.next().is_some() || !matches!(mode, "--inspect" | "--load") {
        usage();
    }

    let image = print_image(&path);
    if mode == "--inspect" {
        return;
    }

    if image.imports.is_empty() {
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
            Err(e) => {
                eprintln!("Wisp map: {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    println!("\nLoading runtime dependencies...");
    match RuntimeLoader::new().load_executable(&path) {
        Ok(main) => {
            println!("  mapped base : 0x{:x}", main.image.base());
            println!("  imports     : resolved for loaded modules");
            println!("  status      : PE loaded/bound; entry execution awaits Windows startup integration");
        }
        Err(e) => {
            eprintln!("Wisp load: {e}");
            std::process::exit(1);
        }
    }
}
