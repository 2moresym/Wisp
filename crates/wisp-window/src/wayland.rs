//! Native Wayland backend using wl_compositor + xdg-shell.
//!
//! The backend deliberately exposes the native wl_display/wl_surface handles
//! needed by future Vulkan WSI integration. It does not use XWayland or a GUI
//! toolkit.

use std::{fs::OpenOptions, io::Write, os::fd::AsFd, path::PathBuf};

use wayland_client::{
    delegate_noop,
    protocol::{wl_keyboard, wl_pointer, wl_registry, wl_seat, wl_shm, wl_surface},
    Connection, Dispatch, EventQueue, QueueHandle, WEnum,
};
use wayland_client::protocol::{wl_buffer, wl_compositor, wl_shm_pool};
use wayland_protocols::xdg::shell::client::{xdg_surface, xdg_toplevel, xdg_wm_base};

use crate::{WindowConfig, WindowError, WindowEvent};

struct State {
    compositor: Option<wl_compositor::WlCompositor>,
    shm: Option<wl_shm::WlShm>,
    wm_base: Option<xdg_wm_base::XdgWmBase>,
    surface: Option<wl_surface::WlSurface>,
    xdg_surface: Option<xdg_surface::XdgSurface>,
    toplevel: Option<xdg_toplevel::XdgToplevel>,
    buffer: Option<wl_buffer::WlBuffer>,
    configured: bool,
    width: u32,
    height: u32,
    events: Vec<WindowEvent>,
}

impl State {
    fn new(width: u32, height: u32) -> Self {
        Self {
            compositor: None,
            shm: None,
            wm_base: None,
            surface: None,
            xdg_surface: None,
            toplevel: None,
            buffer: None,
            configured: false,
            width,
            height,
            events: Vec::new(),
        }
    }

    fn initialize(&mut self, qh: &QueueHandle<Self>, config: &WindowConfig) -> Result<(), WindowError> {
        let compositor = self.compositor.as_ref().ok_or(WindowError::Wayland("wl_compositor missing".into()))?;
        let wm_base = self.wm_base.as_ref().ok_or(WindowError::Wayland("xdg_wm_base missing".into()))?;
        let shm = self.shm.as_ref().ok_or(WindowError::Wayland("wl_shm missing".into()))?;

        let surface = compositor.create_surface(qh, ());
        let xdg_surface = wm_base.get_xdg_surface(&surface, qh, ());
        let toplevel = xdg_surface.get_toplevel(qh, ());
        toplevel.set_title(config.title.clone());
        toplevel.set_app_id("wisp".into());

        let buffer = create_buffer(shm, qh, config.width, config.height)?;

        surface.commit();

        self.surface = Some(surface);
        self.xdg_surface = Some(xdg_surface);
        self.toplevel = Some(toplevel);
        self.buffer = Some(buffer);
        self.width = config.width;
        self.height = config.height;
        Ok(())
    }

    fn present(&mut self) {
        if !self.configured {
            return;
        }
        if let (Some(surface), Some(buffer)) = (&self.surface, &self.buffer) {
            surface.attach(Some(buffer), 0, 0);
            surface.damage(0, 0, self.width as i32, self.height as i32);
            surface.commit();
        }
    }
}

pub struct WaylandWindow {
    connection: Connection,
    event_queue: EventQueue<State>,
    state: State,
}

impl WaylandWindow {
    pub fn new(config: &WindowConfig) -> Result<Self, WindowError> {
        let connection = Connection::connect_to_env()
            .map_err(|e| WindowError::Wayland(e.to_string()))?;
        let mut event_queue = connection.new_event_queue();
        let qh = event_queue.handle();
        let mut state = State::new(config.width, config.height);

        connection.display().get_registry(&qh, ());
        event_queue
            .roundtrip(&mut state)
            .map_err(|e| WindowError::Wayland(e.to_string()))?;

        state.initialize(&qh, config)?;
        Ok(Self { connection, event_queue, state })
    }

    pub fn show(&mut self) -> Result<(), WindowError> {
        self.event_queue
            .roundtrip(&mut self.state)
            .map_err(|e| WindowError::Wayland(e.to_string()))?;
        if !self.state.configured {
            return Err(WindowError::Wayland("compositor did not configure surface".into()));
        }
        self.state.present();
        self.connection
            .flush()
            .map_err(|e| WindowError::Wayland(e.to_string()))?;
        Ok(())
    }

    pub fn poll_events(&mut self) -> Vec<WindowEvent> {
        let _ = self.event_queue.dispatch_pending(&mut self.state);
        let _ = self.connection.flush();
        std::mem::take(&mut self.state.events)
    }

    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), WindowError> {
        if width == 0 || height == 0 {
            return Err(WindowError::Wayland("window dimensions must be non-zero".into()));
        }
        self.state.width = width;
        self.state.height = height;
        Ok(())
    }

    pub fn display(&self) -> wayland_client::protocol::wl_display::WlDisplay {
        self.connection.display()
    }

    pub fn surface(&self) -> Option<&wl_surface::WlSurface> {
        self.state.surface.as_ref()
    }

    pub fn size(&self) -> (u32, u32) {
        (self.state.width, self.state.height)
    }
}

fn create_buffer(
    shm: &wl_shm::WlShm,
    qh: &QueueHandle<State>,
    width: u32,
    height: u32,
) -> Result<wl_buffer::WlBuffer, WindowError> {
    let size = width
        .checked_mul(height)
        .and_then(|v| v.checked_mul(4))
        .ok_or_else(|| WindowError::Wayland("shared-memory buffer size overflow".into()))?;

    let path = PathBuf::from(format!("/dev/shm/wisp-window-{}", std::process::id()));
    let mut file = OpenOptions::new()
        .write(true)
        .read(true)
        .create_new(true)
        .open(&path)
        .map_err(|e| WindowError::Wayland(format!("create shm file: {e}")))?;
    file.set_len(size as u64)
        .map_err(|e| WindowError::Wayland(format!("size shm file: {e}")))?;
    let mut pixels = vec![0u8; size as usize];
    for px in pixels.chunks_exact_mut(4) {
        px.copy_from_slice(&[0x30, 0x30, 0x30, 0xff]);
    }
    file.write_all(&pixels)
        .map_err(|e| WindowError::Wayland(format!("write shm buffer: {e}")))?;
    file.flush()
        .map_err(|e| WindowError::Wayland(format!("flush shm buffer: {e}")))?;
    let _ = std::fs::remove_file(&path);

    let pool = shm.create_pool(file.as_fd(), size as i32, qh, ());
    let buffer = pool.create_buffer(
        0,
        width as i32,
        height as i32,
        (width * 4) as i32,
        wl_shm::Format::Argb8888,
        qh,
        (),
    );
    pool.destroy();
    Ok(buffer)
}

impl Dispatch<wl_registry::WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global { name, interface, version } = event {
            match interface.as_str() {
                "wl_compositor" => {
                    state.compositor = Some(registry.bind(name, version.min(4), qh, ()));
                }
                "wl_shm" => {
                    state.shm = Some(registry.bind(name, version.min(1), qh, ()));
                }
                "xdg_wm_base" => {
                    state.wm_base = Some(registry.bind(name, version.min(6), qh, ()));
                }
                "wl_seat" => {
                    let _: wl_seat::WlSeat = registry.bind(name, version.min(7), qh, ());
                }
                _ => {}
            }
        }
    }
}

impl Dispatch<xdg_wm_base::XdgWmBase, ()> for State {
    fn event(
        _: &mut Self,
        wm_base: &xdg_wm_base::XdgWmBase,
        event: xdg_wm_base::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_wm_base::Event::Ping { serial } = event {
            wm_base.pong(serial);
        }
    }
}

impl Dispatch<xdg_surface::XdgSurface, ()> for State {
    fn event(
        state: &mut Self,
        xdg_surface: &xdg_surface::XdgSurface,
        event: xdg_surface::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_surface::Event::Configure { serial } = event {
            xdg_surface.ack_configure(serial);
            state.configured = true;
            state.present();
        }
    }
}

impl Dispatch<xdg_toplevel::XdgToplevel, ()> for State {
    fn event(
        state: &mut Self,
        _: &xdg_toplevel::XdgToplevel,
        event: xdg_toplevel::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            xdg_toplevel::Event::Configure { width, height, .. } => {
                if width > 0 && height > 0 {
                    let changed = state.width != width as u32 || state.height != height as u32;
                    state.width = width as u32;
                    state.height = height as u32;
                    if changed {
                        state.events.push(WindowEvent::Resized { width: state.width, height: state.height });
                    }
                }
            }
            xdg_toplevel::Event::Close => state.events.push(WindowEvent::CloseRequested),
            _ => {}
        }
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for State {
    fn event(
        _: &mut Self,
        seat: &wl_seat::WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities { capabilities } = event {
            if let WEnum::Value(caps) = capabilities {
                if caps.contains(wl_seat::Capability::Keyboard) {
                    seat.get_keyboard(qh, ());
                }
                if caps.contains(wl_seat::Capability::Pointer) {
                    seat.get_pointer(qh, ());
                }
            }
        }
    }
}

impl Dispatch<wl_keyboard::WlKeyboard, ()> for State {
    fn event(
        state: &mut Self,
        _: &wl_keyboard::WlKeyboard,
        event: wl_keyboard::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_keyboard::Event::Enter { .. } => state.events.push(WindowEvent::Focused(true)),
            wl_keyboard::Event::Leave { .. } => state.events.push(WindowEvent::Focused(false)),
            wl_keyboard::Event::Key { key, state: key_state, .. } => {
                if let WEnum::Value(key_state) = key_state {
                    state.events.push(WindowEvent::KeyboardInput {
                        key,
                        pressed: key_state == wl_keyboard::KeyState::Pressed,
                    });
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_pointer::WlPointer, ()> for State {
    fn event(
        state: &mut Self,
        _: &wl_pointer::WlPointer,
        event: wl_pointer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_pointer::Event::Motion { surface_x, surface_y, .. } => {
                state.events.push(WindowEvent::MouseMoved { x: surface_x, y: surface_y });
            }
            wl_pointer::Event::Button { button, state: button_state, .. } => {
                if let WEnum::Value(button_state) = button_state {
                    state.events.push(WindowEvent::MouseButton {
                        button,
                        pressed: button_state == wl_pointer::ButtonState::Pressed,
                    });
                }
            }
            _ => {}
        }
    }
}

delegate_noop!(State: ignore wl_compositor::WlCompositor);
delegate_noop!(State: ignore wl_surface::WlSurface);
delegate_noop!(State: ignore wl_shm::WlShm);
delegate_noop!(State: ignore wl_shm_pool::WlShmPool);
delegate_noop!(State: ignore wl_buffer::WlBuffer);
