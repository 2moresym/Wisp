#![deny(unsafe_op_in_unsafe_fn)]

mod event;
mod window;

pub use event::WindowEvent;
pub use window::{Window, WindowConfig, WindowError, WindowBackend};
