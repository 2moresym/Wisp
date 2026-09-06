use crate::{wayland::WaylandWindow, WindowEvent};
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
}

pub struct Window {
    backend: WindowBackend,
    config: WindowConfig,
    wayland: Option<WaylandWindow>,
}

impl Window {
    pub fn new(config: WindowConfig) -> Result<Self, WindowError> {
        match detect_backend()? {
            WindowBackend::Wayland => {
                let wayland = WaylandWindow::new(&config)?;
                Ok(Self { backend: WindowBackend::Wayland, config, wayland: Some(wayland) })
            }
            WindowBackend::X11 => Err(WindowError::NotImplemented),
        }
    }

    pub fn backend(&self) -> WindowBackend { self.backend }

    pub fn config(&self) -> &WindowConfig { &self.config }

    pub fn show(&mut self) -> Result<(), WindowError> {
        match self.backend {
            WindowBackend::Wayland => self.wayland.as_mut().unwrap().show(),
            WindowBackend::X11 => Err(WindowError::NotImplemented),
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), WindowError> {
        self.config.width = width;
        self.config.height = height;
        match self.backend {
            WindowBackend::Wayland => self.wayland.as_mut().unwrap().resize(width, height),
            WindowBackend::X11 => Err(WindowError::NotImplemented),
        }
    }

    pub fn poll_events(&mut self) -> Vec<WindowEvent> {
        let events = match self.backend {
            WindowBackend::Wayland => self.wayland.as_mut().map(WaylandWindow::poll_events).unwrap_or_default(),
            WindowBackend::X11 => Vec::new(),
        };
        for event in &events {
            if let WindowEvent::Resized { width, height } = *event {
                self.config.width = width;
                self.config.height = height;
            }
        }
        events
    }

    pub fn native_wayland_display(&self) -> Option<&wayland_client::Display> {
        self.wayland.as_ref().map(WaylandWindow::display)
    }

    pub fn native_wayland_surface(&self) -> Option<&wayland_client::protocol::wl_surface::WlSurface> {
        self.wayland.as_ref().and_then(WaylandWindow::surface)
    }
}

fn detect_backend() -> Result<WindowBackend, WindowError> {
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
