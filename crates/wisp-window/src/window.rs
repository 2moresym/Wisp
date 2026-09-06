use crate::{wayland::WaylandWindow, x11::X11Window, WindowEvent};
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct WindowConfig {
    pub title: String,
    pub width: u32,
    pub height: u32,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self { title: "Wisp Window".into(), width: 1280, height: 720 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowBackend {
    Wayland,
    X11,
}

#[derive(Debug, Error)]
pub enum WindowError {
    #[error("requested window backend is unavailable")]
    BackendUnavailable,
    #[error("window operation is not implemented yet")]
    NotImplemented,
    #[error("Wayland error: {0}")]
    Wayland(String),
    #[error("X11 error: {0}")]
    X11(String),
}

pub struct Window {
    backend: WindowBackend,
    config: WindowConfig,
    wayland: Option<WaylandWindow>,
    x11: Option<X11Window>,
}

impl Window {
    pub fn new(config: WindowConfig) -> Result<Self, WindowError> {
        match detect_backend()? {
            WindowBackend::Wayland => {
                let wayland = WaylandWindow::new(&config)?;
                Ok(Self {
                    backend: WindowBackend::Wayland,
                    config,
                    wayland: Some(wayland),
                    x11: None,
                })
            }
            WindowBackend::X11 => {
                let x11 = X11Window::new(&config)?;
                Ok(Self {
                    backend: WindowBackend::X11,
                    config,
                    wayland: None,
                    x11: Some(x11),
                })
            }
        }
    }

    pub fn backend(&self) -> WindowBackend { self.backend }

    pub fn config(&self) -> &WindowConfig { &self.config }

    pub fn show(&mut self) -> Result<(), WindowError> {
        match self.backend {
            WindowBackend::Wayland => self.wayland.as_mut().unwrap().show(),
            WindowBackend::X11 => self.x11.as_mut().unwrap().show(),
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), WindowError> {
        self.config.width = width;
        self.config.height = height;
        match self.backend {
            WindowBackend::Wayland => self.wayland.as_mut().unwrap().resize(width, height),
            WindowBackend::X11 => self.x11.as_mut().unwrap().resize(width, height),
        }
    }

    pub fn poll_events(&mut self) -> Vec<WindowEvent> {
        let events = match self.backend {
            WindowBackend::Wayland => self.wayland.as_mut().unwrap().poll_events(),
            WindowBackend::X11 => self.x11.as_mut().unwrap().poll_events(),
        };
        for event in &events {
            if let WindowEvent::Resized { width, height } = *event {
                self.config.width = width;
                self.config.height = height;
            }
        }
        events
    }

    pub fn native_wayland_display(&self) -> Option<wayland_client::protocol::wl_display::WlDisplay> {
        self.wayland.as_ref().map(WaylandWindow::display)
    }

    pub fn native_wayland_surface(&self) -> Option<&wayland_client::protocol::wl_surface::WlSurface> {
        self.wayland.as_ref().and_then(WaylandWindow::surface)
    }

    pub fn native_x11_connection(&self) -> Option<&x11rb::rust_connection::RustConnection> {
        self.x11.as_ref().map(X11Window::connection)
    }

    pub fn native_x11_window(&self) -> Option<x11rb::protocol::xproto::Window> {
        self.x11.as_ref().map(X11Window::window)
    }

    pub fn native_x11_screen(&self) -> Option<usize> {
        self.x11.as_ref().map(X11Window::screen_num)
    }
}

fn detect_backend() -> Result<WindowBackend, WindowError> {
    match std::env::var("WISP_WINDOW_BACKEND").ok().as_deref() {
        Some("wayland") => {
            if std::env::var_os("WAYLAND_DISPLAY").is_some() {
                return Ok(WindowBackend::Wayland);
            }
            return Err(WindowError::BackendUnavailable);
        }
        Some("x11") => {
            if std::env::var_os("DISPLAY").is_some() {
                return Ok(WindowBackend::X11);
            }
            return Err(WindowError::BackendUnavailable);
        }
        Some(_) => return Err(WindowError::BackendUnavailable),
        None => {}
    }

    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        return Ok(WindowBackend::Wayland);
    }
    if std::env::var_os("DISPLAY").is_some() {
        return Ok(WindowBackend::X11);
    }
    Err(WindowError::BackendUnavailable)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_window_config_is_sane() {
        let config = WindowConfig::default();
        assert_eq!((config.width, config.height), (1280, 720));
        assert_eq!(config.title, "Wisp Window");
    }
}
