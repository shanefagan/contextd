//! Official Client Library for contextd (Context Daemon)
//!
//! Provides type-safe Rust client interfaces to subscribe to ambient lighting vibes,
//! control lighting contexts, or query active game and system state from `contextd`.

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

pub use control::VarlinkClientInterface as ControlInterface;
pub use observer::VarlinkClientInterface as ObserverInterface;

use log::{info, warn};
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

    pub fn scaled(&self, brightness: u8) -> (u8, u8, u8) {
        let alpha_factor = (self.a as u32 * brightness as u32) as f32 / 65025.0;
        let r = (self.r as f32 * alpha_factor).round() as u8;
        let g = (self.g as f32 * alpha_factor).round() as u8;
        let b = (self.b as f32 * alpha_factor).round() as u8;
        (r, g, b)
    }
}

/// Client for subscribing to ambient lighting vibe updates (Read-Only)
pub struct ObserverClient {
    socket_path: String,
}

impl ObserverClient {
    pub fn new(socket_path: Option<String>) -> Self {
        Self {
            socket_path: socket_path.unwrap_or_else(|| DEFAULT_OBSERVER_SOCKET.to_string()),
        }
    }

    pub fn subscribe_vibe<F>(&self, mut on_vibe: F) -> Result<(), ClientError>
    where
        F: FnMut(RgbaColor, Option<Vec<RgbaColor>>),
    {
        let address = format!("unix:{}", self.socket_path);

        loop {
            info!("Connecting to contextd Observer socket at {}", address);
            match varlink::Connection::with_address(&address) {
                Ok(conn) => {
                    let mut client = observer::VarlinkClient::new(conn);
                    match client.subscribe_lighting_context().more() {
                        Ok(stream) => {
                            info!("Successfully subscribed to contextd lighting vibe updates.");
                            for reply in stream {
                                match reply {
                                    Ok(evt) => {
                                        let main_color = RgbaColor::new(
                                            evt.main_color.r as u8,
                                            evt.main_color.g as u8,
                                            evt.main_color.b as u8,
                                            evt.main_color.a as u8,
                                        );

                                        let matrix_data = evt.matrix.map(|m| {
                                            m.data
                                                .into_iter()
                                                .map(|c| {
                                                    RgbaColor::new(
                                                        c.r as u8, c.g as u8, c.b as u8, c.a as u8,
                                                    )
                                                })
                                                .collect()
                                        });

                                        on_vibe(main_color, matrix_data);
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

            thread::sleep(Duration::from_secs(3));
        }
    }
}

/// Client for updating contextd lighting state (Control Interface)
pub struct ControlClient {
    socket_path: String,
}

impl ControlClient {
    pub fn new(socket_path: Option<String>) -> Self {
        Self {
            socket_path: socket_path.unwrap_or_else(|| DEFAULT_CONTROL_SOCKET.to_string()),
        }
    }

    pub fn set_lighting_context(
        &self,
        main_color: Option<RgbaColor>,
        matrix: Option<(usize, usize, Vec<RgbaColor>)>,
    ) -> Result<(), ClientError> {
        let address = format!("unix:{}", self.socket_path);
        let conn = varlink::Connection::with_address(&address)?;
        let mut client = control::VarlinkClient::new(conn);

        let varlink_main_color = main_color.map(|c| control::Color {
            r: c.r as i64,
            g: c.g as i64,
            b: c.b as i64,
            a: c.a as i64,
        });

        let varlink_matrix = matrix.map(|(width, height, data)| control::Matrix {
            width: width as i64,
            height: height as i64,
            data: data
                .into_iter()
                .map(|c| control::Color {
                    r: c.r as i64,
                    g: c.g as i64,
                    b: c.b as i64,
                    a: c.a as i64,
                })
                .collect(),
        });

        client
            .set_lighting_context(varlink_main_color, varlink_matrix)
            .call()?;

        Ok(())
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
}
