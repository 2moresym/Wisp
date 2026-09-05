use crate::WindowEvent;
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct WindowConfig {
    pub title: String,
    pub width: u32,
    pub height: u32,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            title: "Wisp Window".into(),
            width: 1280,
            height: 720,
        }
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
}

pub struct Window {
    backend: WindowBackend,
    config: WindowConfig,
}

impl Window {
    pub fn new(config: WindowConfig) -> Result<Self, WindowError> {
        let backend = detect_backend()?;
        Ok(Self { backend, config })
    }

    pub fn backend(&self) -> WindowBackend {
        self.backend
    }

    pub fn config(&self) -> &WindowConfig {
        &self.config
    }

    pub fn show(&mut self) -> Result<(), WindowError> {
        Err(WindowError::NotImplemented)
    }

    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), WindowError> {
        self.config.width = width;
        self.config.height = height;
        Err(WindowError::NotImplemented)
    }

    pub fn poll_events(&mut self) -> Vec<WindowEvent> {
        Vec::new()
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
