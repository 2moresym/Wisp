#![deny(unsafe_op_in_unsafe_fn)]

mod event;
mod window;
pub mod wayland;
pub mod x11;

pub use event::WindowEvent;
pub use window::{Window, WindowBackend, WindowConfig, WindowError};
