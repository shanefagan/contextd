//! Official client library for contextd (Context Daemon).
//!
//! Provides type-safe clients for the three contextd sockets:
//!
//! - [`CoreClient`]: games, devices, diagnostics and controller hints
//!   (`com.performativenonsense.contextd`).
//! - [`ObserverClient`]: read and subscribe to the ambient lighting context
//!   (`com.performativenonsense.contextd.rgb.Observer`).
//! - [`ControlClient`]: change the ambient lighting context
//!   (`com.performativenonsense.contextd.rgb.Control`).
//!
//! The generated Varlink modules ([`contextd`], [`observer`], [`control`])
//! are public too, and the daemon itself is built on them.

#[allow(clippy::all, non_snake_case, non_camel_case_types, unused_imports)]
pub mod contextd {
    include!(concat!(env!("OUT_DIR"), "/contextd.rs"));
}

#[allow(clippy::all, non_snake_case, non_camel_case_types, unused_imports)]
pub mod control {
    include!(concat!(env!("OUT_DIR"), "/control.rs"));
}

#[allow(clippy::all, non_snake_case, non_camel_case_types, unused_imports)]
pub mod observer {
    include!(concat!(env!("OUT_DIR"), "/observer.rs"));
}

pub use contextd::VarlinkClientInterface as CoreInterface;
pub use contextd::{Controller, Device, Diagnostics, Game, Gpu};
pub use control::VarlinkClientInterface as ControlInterface;
pub use observer::VarlinkClientInterface as ObserverInterface;

use log::{info, warn};
use std::ops::ControlFlow;
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::Duration;
use thiserror::Error;

pub const DEFAULT_OBSERVER_SOCKET: &str = "/run/contextd/public/contextd-rgb-observer.socket";
pub const DEFAULT_CONTROL_SOCKET: &str = "/run/contextd/private/contextd-rgb-control.socket";
pub const DEFAULT_CORE_SOCKET: &str = "/run/contextd/public/contextd.socket";

#[derive(Error, Debug)]
pub enum ClientError {
    #[error("Varlink connection error: {0}")]
    Varlink(#[from] varlink::Error),
    #[error("Varlink Control error: {0}")]
    Control(#[from] control::Error),
    #[error("Varlink Observer error: {0}")]
    Observer(#[from] observer::Error),
    #[error("Varlink Core error: {0}")]
    Core(#[from] contextd::Error),
    #[error("I/O Error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Client error: {0}")]
    General(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RgbaColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl RgbaColor {
    pub fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Applies alpha and a 0-255 brightness to the color, returning RGB.
    pub fn scaled(&self, brightness: u8) -> (u8, u8, u8) {
        let alpha_factor = (self.a as u32 * brightness as u32) as f32 / 65025.0;
        let r = (self.r as f32 * alpha_factor).round() as u8;
        let g = (self.g as f32 * alpha_factor).round() as u8;
        let b = (self.b as f32 * alpha_factor).round() as u8;
        (r, g, b)
    }

    fn from_components(r: i64, g: i64, b: i64, a: i64) -> Self {
        let c = |v: i64| v.clamp(0, 255) as u8;
        Self::new(c(r), c(g), c(b), c(a))
    }
}

impl From<observer::Color> for RgbaColor {
    fn from(c: observer::Color) -> Self {
        Self::from_components(c.r, c.g, c.b, c.a)
    }
}

impl From<RgbaColor> for control::Color {
    fn from(c: RgbaColor) -> Self {
        control::Color {
            r: c.r as i64,
            g: c.g as i64,
            b: c.b as i64,
            a: c.a as i64,
        }
    }
}

/// A `width` x `height` grid of colors, row-major.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColorMatrix {
    pub width: usize,
    pub height: usize,
    pub data: Vec<RgbaColor>,
}

impl From<observer::Matrix> for ColorMatrix {
    fn from(m: observer::Matrix) -> Self {
        Self {
            width: m.width.max(0) as usize,
            height: m.height.max(0) as usize,
            data: m.data.into_iter().map(RgbaColor::from).collect(),
        }
    }
}

impl From<ColorMatrix> for control::Matrix {
    fn from(m: ColorMatrix) -> Self {
        control::Matrix {
            width: m.width as i64,
            height: m.height as i64,
            data: m.data.into_iter().map(control::Color::from).collect(),
        }
    }
}

/// The full lighting context published by contextd.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LightingVibe {
    pub main_color: RgbaColor,
    pub matrix: Option<ColorMatrix>,
}

impl LightingVibe {
    fn new(main_color: observer::Color, matrix: Option<observer::Matrix>) -> Self {
        Self {
            main_color: main_color.into(),
            matrix: matrix.map(ColorMatrix::from),
        }
    }
}

/// Errors that mean the connection is unusable, as opposed to an error
/// reply from the daemon.
trait TransportError {
    fn is_transport(&self) -> bool;
}

impl TransportError for control::Error {
    fn is_transport(&self) -> bool {
        matches!(self.kind(), control::ErrorKind::Varlink_Error)
    }
}

impl TransportError for observer::Error {
    fn is_transport(&self) -> bool {
        matches!(self.kind(), observer::ErrorKind::Varlink_Error)
    }
}

impl TransportError for contextd::Error {
    fn is_transport(&self) -> bool {
        matches!(self.kind(), contextd::ErrorKind::Varlink_Error)
    }
}

/// A lazily opened connection that is reused across calls and reopened
/// once if the daemon restarted in between.
struct Persistent<C> {
    address: String,
    client: Mutex<Option<C>>,
    ctor: fn(Arc<RwLock<varlink::Connection>>) -> C,
}

impl<C> Persistent<C> {
    fn new(socket_path: &str, ctor: fn(Arc<RwLock<varlink::Connection>>) -> C) -> Self {
        Self {
            address: format!("unix:{socket_path}"),
            client: Mutex::new(None),
            ctor,
        }
    }

    fn call<T, E>(&self, f: impl Fn(&mut C) -> Result<T, E>) -> Result<T, ClientError>
    where
        E: TransportError + Into<ClientError>,
    {
        let mut guard = self.client.lock().unwrap_or_else(|e| e.into_inner());
        let mut retried = false;
        loop {
            let client = match guard.as_mut() {
                Some(c) => c,
                None => {
                    let conn = varlink::Connection::with_address(&self.address)?;
                    guard.insert((self.ctor)(conn))
                }
            };
            match f(client) {
                Ok(v) => return Ok(v),
                Err(e) if e.is_transport() => {
                    *guard = None;
                    if retried {
                        return Err(e.into());
                    }
                    retried = true;
                }
                Err(e) => return Err(e.into()),
            }
        }
    }
}

/// Client for the core interface: games, devices, diagnostics and hints.
pub struct CoreClient {
    conn: Persistent<contextd::VarlinkClient>,
}

impl CoreClient {
    pub fn new(socket_path: Option<String>) -> Self {
        let path = socket_path.unwrap_or_else(|| DEFAULT_CORE_SOCKET.to_string());
        Self {
            conn: Persistent::new(&path, contextd::VarlinkClient::new),
        }
    }

    pub fn active_game(&self) -> Result<Option<Game>, ClientError> {
        self.conn
            .call(|c| c.get_active_game().call().map(|r| r.game))
    }

    pub fn running_games(&self) -> Result<Vec<Game>, ClientError> {
        self.conn
            .call(|c| c.list_running_games().call().map(|r| r.games))
    }

    pub fn installed_games(&self) -> Result<Vec<Game>, ClientError> {
        self.conn
            .call(|c| c.list_installed_games().call().map(|r| r.games))
    }

    pub fn devices(&self) -> Result<Vec<Device>, ClientError> {
        self.conn
            .call(|c| c.list_devices().call().map(|r| r.devices))
    }

    pub fn rgb_devices(&self) -> Result<Vec<Device>, ClientError> {
        self.conn
            .call(|c| c.list_rgbdevices().call().map(|r| r.devices))
    }

    pub fn diagnostics(&self) -> Result<Diagnostics, ClientError> {
        self.conn
            .call(|c| c.get_diagnostics().call().map(|r| r.diagnostics))
    }

    pub fn controllers(&self) -> Result<Vec<Controller>, ClientError> {
        self.conn
            .call(|c| c.list_controllers().call().map(|r| r.controllers))
    }

    /// Registers a hint. `controller.pid` must be this process or one of
    /// its ancestors.
    pub fn register_controller(&self, controller: Controller) -> Result<(), ClientError> {
        self.conn
            .call(|c| c.register_controller(controller.clone()).call().map(|_| ()))
    }

    pub fn unregister_controller(&self, pid: i64) -> Result<(), ClientError> {
        self.conn
            .call(|c| c.unregister_controller(pid).call().map(|_| ()))
    }
}

/// Client for reading and subscribing to the lighting context (read-only).
pub struct ObserverClient {
    socket_path: String,
    reconnect_delay: Duration,
}

impl ObserverClient {
    pub fn new(socket_path: Option<String>) -> Self {
        Self {
            socket_path: socket_path.unwrap_or_else(|| DEFAULT_OBSERVER_SOCKET.to_string()),
            reconnect_delay: Duration::from_secs(3),
        }
    }

    /// Sets how long [`ObserverClient::subscribe`] waits before reconnecting
    /// after the daemon goes away. Defaults to 3 seconds.
    pub fn with_reconnect_delay(mut self, delay: Duration) -> Self {
        self.reconnect_delay = delay;
        self
    }

    fn address(&self) -> String {
        format!("unix:{}", self.socket_path)
    }

    /// Returns the current lighting context.
    pub fn get(&self) -> Result<LightingVibe, ClientError> {
        let conn = varlink::Connection::with_address(&self.address())?;
        let mut client = observer::VarlinkClient::new(conn);
        let reply = client.get_lighting_context().call()?;
        Ok(LightingVibe::new(reply.main_color, reply.matrix))
    }

    /// Streams lighting updates to `on_vibe`, reconnecting automatically if
    /// the daemon restarts. Returns `Ok(())` when the callback returns
    /// [`ControlFlow::Break`].
    pub fn subscribe<F>(&self, mut on_vibe: F) -> Result<(), ClientError>
    where
        F: FnMut(LightingVibe) -> ControlFlow<()>,
    {
        let address = self.address();
        loop {
            info!("Connecting to contextd Observer socket at {}", address);
            match varlink::Connection::with_address(&address) {
                Ok(conn) => {
                    let mut client = observer::VarlinkClient::new(conn);
                    match client.subscribe_lighting_context().more() {
                        Ok(stream) => {
                            info!("Subscribed to contextd lighting updates.");
                            for reply in stream {
                                match reply {
                                    Ok(evt) => {
                                        let vibe = LightingVibe::new(evt.main_color, evt.matrix);
                                        if on_vibe(vibe).is_break() {
                                            return Ok(());
                                        }
                                    }
                                    Err(e) => {
                                        warn!("Error in observer stream: {}", e);
                                        break;
                                    }
                                }
                            }
                        }
                        Err(e) => warn!("Failed to call SubscribeLightingContext: {}", e),
                    }
                }
                Err(e) => warn!("Could not connect to contextd observer socket: {}", e),
            }

            thread::sleep(self.reconnect_delay);
        }
    }

    /// Streams updates forever. Prefer [`ObserverClient::subscribe`], which
    /// can be stopped and keeps the matrix dimensions.
    #[deprecated(
        since = "0.10.0",
        note = "use `subscribe`, which can be stopped and keeps matrix dimensions"
    )]
    pub fn subscribe_vibe<F>(&self, mut on_vibe: F) -> Result<(), ClientError>
    where
        F: FnMut(RgbaColor, Option<Vec<RgbaColor>>),
    {
        self.subscribe(|vibe| {
            on_vibe(vibe.main_color, vibe.matrix.map(|m| m.data));
            ControlFlow::Continue(())
        })
    }
}

/// Client for updating the lighting context (control interface). The
/// connection is kept open between calls.
pub struct ControlClient {
    conn: Persistent<control::VarlinkClient>,
}

impl ControlClient {
    pub fn new(socket_path: Option<String>) -> Self {
        let path = socket_path.unwrap_or_else(|| DEFAULT_CONTROL_SOCKET.to_string());
        Self {
            conn: Persistent::new(&path, control::VarlinkClient::new),
        }
    }

    /// Sets the main color and/or matrix. Fields passed as `None` keep their
    /// current value. `matrix` is `(width, height, row-major data)`.
    pub fn set_lighting_context(
        &self,
        main_color: Option<RgbaColor>,
        matrix: Option<(usize, usize, Vec<RgbaColor>)>,
    ) -> Result<(), ClientError> {
        let matrix = matrix.map(|(width, height, data)| ColorMatrix {
            width,
            height,
            data,
        });
        self.send(main_color, matrix, None)
    }

    pub fn set_main_color(&self, color: RgbaColor) -> Result<(), ClientError> {
        self.send(Some(color), None, None)
    }

    pub fn set_matrix(&self, matrix: ColorMatrix) -> Result<(), ClientError> {
        self.send(None, Some(matrix), None)
    }

    /// Removes the current matrix, leaving only the main color.
    pub fn clear_matrix(&self) -> Result<(), ClientError> {
        self.send(None, None, Some(true))
    }

    fn send(
        &self,
        main_color: Option<RgbaColor>,
        matrix: Option<ColorMatrix>,
        clear_matrix: Option<bool>,
    ) -> Result<(), ClientError> {
        let main_color = main_color.map(control::Color::from);
        let matrix = matrix.map(control::Matrix::from);
        self.conn.call(|c| {
            c.set_lighting_context(main_color.clone(), matrix.clone(), clear_matrix)
                .call()
                .map(|_| ())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rgba_color_scaling() {
        let color = RgbaColor::new(255, 128, 64, 255);
        assert_eq!(color.scaled(255), (255, 128, 64));
        assert_eq!(color.scaled(128), (128, 64, 32));
    }

    #[test]
    fn out_of_range_components_are_clamped() {
        let c = RgbaColor::from(observer::Color {
            r: -5,
            g: 300,
            b: 7,
            a: 255,
        });
        assert_eq!(c, RgbaColor::new(0, 255, 7, 255));
    }

    #[test]
    fn matrix_keeps_dimensions() {
        let m = ColorMatrix::from(observer::Matrix {
            width: 2,
            height: 1,
            data: vec![
                observer::Color {
                    r: 1,
                    g: 2,
                    b: 3,
                    a: 4,
                };
                2
            ],
        });
        assert_eq!((m.width, m.height, m.data.len()), (2, 1, 2));
    }
}
