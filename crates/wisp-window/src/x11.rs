//! X11 backend boundary.
//!
//! The implementation will expose an X11 window and native handle suitable
//! for Vulkan Xlib/XCB WSI without making X11 a requirement on Wayland hosts.

pub struct X11Window;
