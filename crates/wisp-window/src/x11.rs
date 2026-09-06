//! Minimal native X11 backend.
//!
//! This backend is intentionally small: create a top-level window, select the
//! game-relevant event classes, process WM_DELETE_WINDOW/configure/input events,
//! and expose the X11 protocol connection plus window ID for future WSI paths.

use x11rb::{
    connection::Connection,
    protocol::{xproto, Event},
    rust_connection::RustConnection,
};

use crate::{WindowConfig, WindowError, WindowEvent};

pub struct X11Window {
    connection: RustConnection,
    screen_num: usize,
    window: xproto::Window,
    wm_delete_window: xproto::Atom,
    width: u32,
    height: u32,
    events: Vec<WindowEvent>,
}

impl X11Window {
    pub fn new(config: &WindowConfig) -> Result<Self, WindowError> {
        if config.width == 0 || config.height == 0 {
            return Err(WindowError::X11("window dimensions must be non-zero".into()));
        }

        let (connection, screen_num) =
            x11rb::connect(None).map_err(|e| WindowError::X11(e.to_string()))?;
        let screen = &connection.setup().roots[screen_num];

        let window = connection
            .generate_id()
            .map_err(|e| WindowError::X11(e.to_string()))?;
        let event_mask = xproto::EventMask::EXPOSURE
            | xproto::EventMask::STRUCTURE_NOTIFY
            | xproto::EventMask::FOCUS_CHANGE
            | xproto::EventMask::KEY_PRESS
            | xproto::EventMask::KEY_RELEASE
            | xproto::EventMask::BUTTON_PRESS
            | xproto::EventMask::BUTTON_RELEASE
            | xproto::EventMask::POINTER_MOTION;

        connection
            .create_window(
                x11rb::COPY_DEPTH_FROM_PARENT,
                window,
                screen.root,
                0,
                0,
                config.width.min(u16::MAX as u32) as u16,
                config.height.min(u16::MAX as u32) as u16,
                0,
                xproto::WindowClass::INPUT_OUTPUT,
                screen.root_visual,
                &xproto::CreateWindowAux::new()
                    .background_pixel(screen.black_pixel)
                    .event_mask(event_mask),
            )
            .map_err(|e| WindowError::X11(e.to_string()))?;

        let wm_protocols = intern_atom(&connection, b"WM_PROTOCOLS")?;
        let wm_delete_window = intern_atom(&connection, b"WM_DELETE_WINDOW")?;
        connection
            .change_property32(
                xproto::PropMode::REPLACE,
                window,
                wm_protocols,
                xproto::AtomEnum::ATOM,
                &[wm_delete_window],
            )
            .map_err(|e| WindowError::X11(e.to_string()))?;

        connection
            .change_property8(
                xproto::PropMode::REPLACE,
                window,
                xproto::AtomEnum::WM_NAME,
                xproto::AtomEnum::STRING,
                config.title.as_bytes(),
            )
            .map_err(|e| WindowError::X11(e.to_string()))?;

        connection
            .map_window(window)
            .map_err(|e| WindowError::X11(e.to_string()))?;
        connection
            .flush()
            .map_err(|e| WindowError::X11(e.to_string()))?;

        Ok(Self {
            connection,
            screen_num,
            window,
            wm_delete_window,
            width: config.width,
            height: config.height,
            events: Vec::new(),
        })
    }

    pub fn show(&mut self) -> Result<(), WindowError> {
        self.connection
            .map_window(self.window)
            .map_err(|e| WindowError::X11(e.to_string()))?;
        self.connection
            .flush()
            .map_err(|e| WindowError::X11(e.to_string()))?;
        Ok(())
    }

    pub fn poll_events(&mut self) -> Vec<WindowEvent> {
        loop {
            match self.connection.poll_for_event() {
                Ok(Some(event)) => self.handle_event(event),
                Ok(None) => break,
                Err(_) => break,
            }
        }
        std::mem::take(&mut self.events)
    }

    fn handle_event(&mut self, event: Event) {
        match event {
            Event::ClientMessage(event)
                if event.format == 32
                    && event.data.as_data32()[0] == self.wm_delete_window =>
            {
                self.events.push(WindowEvent::CloseRequested);
            }
            Event::ConfigureNotify(event) if event.window == self.window => {
                let width = event.width as u32;
                let height = event.height as u32;
                if width != 0 && height != 0 && (width != self.width || height != self.height) {
                    self.width = width;
                    self.height = height;
                    self.events.push(WindowEvent::Resized { width, height });
                }
            }
            Event::FocusIn(_) => self.events.push(WindowEvent::Focused(true)),
            Event::FocusOut(_) => self.events.push(WindowEvent::Focused(false)),
            Event::KeyPress(event) => self.events.push(WindowEvent::KeyboardInput {
                key: event.detail.into(),
                pressed: true,
            }),
            Event::KeyRelease(event) => self.events.push(WindowEvent::KeyboardInput {
                key: event.detail.into(),
                pressed: false,
            }),
            Event::MotionNotify(event) if event.event == self.window => {
                self.events.push(WindowEvent::MouseMoved {
                    x: event.event_x.into(),
                    y: event.event_y.into(),
                });
            }
            Event::ButtonPress(event) if event.event == self.window => {
                self.events.push(WindowEvent::MouseButton {
                    button: event.detail.into(),
                    pressed: true,
                });
            }
            Event::ButtonRelease(event) if event.event == self.window => {
                self.events.push(WindowEvent::MouseButton {
                    button: event.detail.into(),
                    pressed: false,
                });
            }
            _ => {}
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), WindowError> {
        if width == 0 || height == 0 {
            return Err(WindowError::X11("window dimensions must be non-zero".into()));
        }
        self.connection
            .configure_window(
                self.window,
                &xproto::ConfigureWindowAux::new()
                    .width(width)
                    .height(height),
            )
            .map_err(|e| WindowError::X11(e.to_string()))?;
        self.width = width;
        self.height = height;
        self.connection
            .flush()
            .map_err(|e| WindowError::X11(e.to_string()))?;
        Ok(())
    }

    pub fn connection(&self) -> &RustConnection {
        &self.connection
    }

    pub fn screen_num(&self) -> usize {
        self.screen_num
    }

    pub fn window(&self) -> xproto::Window {
        self.window
    }

    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}

impl Drop for X11Window {
    fn drop(&mut self) {
        let _ = self.connection.destroy_window(self.window);
        let _ = self.connection.flush();
    }
}

fn intern_atom(connection: &RustConnection, name: &[u8]) -> Result<xproto::Atom, WindowError> {
    let reply = connection
        .intern_atom(false, name)
        .map_err(|e| WindowError::X11(e.to_string()))?
        .reply()
        .map_err(|e| WindowError::X11(e.to_string()))?;
    Ok(reply.atom)
}
