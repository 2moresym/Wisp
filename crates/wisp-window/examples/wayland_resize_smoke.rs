use std::{thread, time::Duration};

use wisp_window::{Window, WindowBackend, WindowConfig, WindowEvent};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut window = Window::new(WindowConfig {
        title: "Wisp Wayland Resize Smoke Test".into(),
        width: 800,
        height: 600,
    })?;

    if window.backend() != WindowBackend::Wayland {
        return Err("Wayland resize smoke test requires WAYLAND_DISPLAY".into());
    }

    window.show()?;
    println!("resize smoke test running; resize the window and close it to finish");

    loop {
        for event in window.poll_events() {
            match event {
                WindowEvent::CloseRequested => return Ok(()),
                WindowEvent::Resized { width, height } => {
                    println!("received compositor resize: {width}x{height}");
                }
                _ => {}
            }
        }
        thread::sleep(Duration::from_millis(10));
    }
}
