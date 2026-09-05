#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowEvent {
    CloseRequested,
    Resized { width: u32, height: u32 },
    Focused(bool),
}
