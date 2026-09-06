#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WindowEvent {
    CloseRequested,
    Resized { width: u32, height: u32 },
    Focused(bool),
    KeyboardInput { key: u32, pressed: bool },
    MouseMoved { x: f64, y: f64 },
    MouseButton { button: u32, pressed: bool },
}
