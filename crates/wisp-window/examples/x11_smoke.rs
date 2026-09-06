use std::{thread, time::Duration};

use wisp_window::{Window, WindowBackend, WindowConfig, WindowEvent};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var_os("DISPLAY").is_none() {
        return Err("X11 smoke test requires DISPLAY (X11/XWayland)".into());
    }

    std::env::set_var("WISP_WINDOW_BACKEND", "x11");

    let mut window = Window::new(WindowConfig {
        title: "Wisp X11 Smoke Test".into(),
        width: 800,
        height: 600,
    })?;

    if window.backend() != WindowBackend::X11 {
        return Err("failed to select X11 backend".into());
    }

    window.show()?;
    println!("Wisp X11 smoke test running at {:?}", window.config());

    for _ in 0..500 {
        for event in window.poll_events() {
            match event {
                WindowEvent::CloseRequested => return Ok(()),
                WindowEvent::Resized { width, height } => {
                    println!("resized to {width}x{height}");
                }
                _ => {}
            }
        }
        thread::sleep(Duration::from_millis(10));
    }

    Ok(())
}
