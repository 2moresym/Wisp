use std::{thread, time::Duration};

use wisp_window::{Window, WindowBackend, WindowConfig, WindowEvent};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut window = Window::new(WindowConfig {
        title: "Wisp Wayland Smoke Test".into(),
        width: 800,
        height: 600,
    })?;

    if window.backend() != WindowBackend::Wayland {
        return Err("Wayland smoke test requires WAYLAND_DISPLAY".into());
    }

    window.show()?;
    println!("Wisp Wayland smoke test running at {:?}", window.config());

    for _ in 0..500 {
        for event in window.poll_events() {
            match event {
                WindowEvent::CloseRequested => return Ok(()),
                WindowEvent::Resized { width, height } => println!("resized to {width}x{height}"),
                _ => {}
            }
        }
        thread::sleep(Duration::from_millis(10));
    }

    Ok(())
}
