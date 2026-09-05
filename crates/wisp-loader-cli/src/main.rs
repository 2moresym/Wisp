use std::env;

fn main() {
    let Some(path) = env::args().nth(1) else {
        eprintln!("usage: wisp <game.exe>");
        std::process::exit(2);
    };
    match wisp_pe_loader::inspect(&path) {
        Ok(image) => println!("Wisp PE64: base=0x{:x} image=0x{:x} entry=0x{:x}", image.image_base, image.size_of_image, image.entry_rva),
        Err(e) => { eprintln!("Wisp: {e}"); std::process::exit(1); }
    }
}
