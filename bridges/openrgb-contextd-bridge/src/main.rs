//! OpenRGB SDK Bridge Daemon for contextd
//!
//! Listens for OpenRGB SDK TCP client connections on port 6742 (127.0.0.1:6742).
//! Translates OpenRGB LED color update requests into contextd `SetLightingContext`
//! Varlink calls on `/run/contextd/private/contextd-rgb-control.socket`.

mod protocol;

use contextd_client::{ControlClient, RgbaColor};
use log::{error, info, warn};
use protocol::*;
use std::env;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let port: u16 = env::var("OPENRGB_SERVER_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(OPENRGB_DEFAULT_PORT);

    let bind_addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(&bind_addr).await?;

    info!(
        "OpenRGB SDK Bridge listening on {} (forwarding to contextd)...",
        bind_addr
    );

    let control_socket = env::var("CONTEXTD_CONTROL_SOCKET").ok();
    let client = Arc::new(ControlClient::new(control_socket));

    loop {
        match listener.accept().await {
            Ok((socket, addr)) => {
                info!("Accepted OpenRGB SDK client connection from {}", addr);
                let client_ref = Arc::clone(&client);
                tokio::spawn(async move {
                    if let Err(e) = handle_client(socket, client_ref).await {
                        warn!("OpenRGB client connection error: {}", e);
                    }
                });
            }
            Err(e) => {
                error!("TCP accept error: {}", e);
            }
        }
    }
}

async fn handle_client(mut socket: TcpStream, client: Arc<ControlClient>) -> anyhow::Result<()> {
    let mut protocol_version: u32 = 0;

    loop {
        let mut header_buf = [0u8; 16];
        if socket.read_exact(&mut header_buf).await.is_err() {
            break; // Client disconnected
        }

        let mut cursor = std::io::Cursor::new(&header_buf[..]);
        let header = match Header::read_from(&mut cursor)? {
            Some(h) => h,
            None => break,
        };

        let mut payload = vec![0u8; header.pkt_size as usize];
        if header.pkt_size > 0 {
            socket.read_exact(&mut payload).await?;
        }

        match header.pkt_id {
            PKT_REQUEST_PROTOCOL_VERSION => {
                let mut req_ver = PROTOCOL_VERSION;
                if payload.len() >= 4 {
                    req_ver = u32::from_le_bytes(payload[0..4].try_into().unwrap());
                }
                protocol_version = req_ver.min(PROTOCOL_VERSION);
                info!("Client requested protocol version {}", protocol_version);

                let resp_hdr = Header {
                    dev_id: 0,
                    pkt_id: PKT_REQUEST_PROTOCOL_VERSION,
                    pkt_size: 4,
                };
                let mut resp = Vec::new();
                resp_hdr.write_to(&mut resp)?;
                resp.extend_from_slice(&protocol_version.to_le_bytes());
                socket.write_all(&resp).await?;
            }
            PKT_REQUEST_CONTROLLER_COUNT => {
                let count: u32 = 1; // 1 Virtual contextd Controller
                let resp_hdr = Header {
                    dev_id: 0,
                    pkt_id: PKT_REQUEST_CONTROLLER_COUNT,
                    pkt_size: 4,
                };
                let mut resp = Vec::new();
                resp_hdr.write_to(&mut resp)?;
                resp.extend_from_slice(&count.to_le_bytes());
                socket.write_all(&resp).await?;
            }
            PKT_REQUEST_CONTROLLER_DATA => {
                let data = build_mock_controller_data(protocol_version);
                let resp_hdr = Header {
                    dev_id: header.dev_id,
                    pkt_id: PKT_REQUEST_CONTROLLER_DATA,
                    pkt_size: data.len() as u32,
                };
                let mut resp = Vec::new();
                resp_hdr.write_to(&mut resp)?;
                resp.extend_from_slice(&data);
                socket.write_all(&resp).await?;
            }
            PKT_SET_CLIENT_NAME => {
                if let Ok(name) = String::from_utf8(payload) {
                    info!(
                        "OpenRGB Client registered name: {}",
                        name.trim_matches('\0')
                    );
                }
            }
            PKT_UPDATE_LEDS
            | PKT_UPDATE_ZONE_LEDS
            | PKT_UPDATE_SINGLE_LED
            | PKT_SET_CUSTOM_MODE => {
                // Parse color bytes from OpenRGB payload:
                // Format: u16 count, followed by [R, G, B, 0] or [R, G, B] per LED
                if payload.len() >= 6 {
                    let r = payload[2];
                    let g = payload[3];
                    let b = payload[4];

                    let color = RgbaColor::new(r, g, b, 255);
                    let client_c = Arc::clone(&client);

                    // Forward to contextd on blocking thread pool
                    tokio::task::spawn_blocking(move || {
                        if let Err(e) = client_c.set_lighting_context(Some(color), None) {
                            warn!(
                                "Failed to forward OpenRGB lighting context to contextd: {}",
                                e
                            );
                        }
                    });
                }
            }
            _ => {
                // Ignore unhandled packets or respond with OK
            }
        }
    }

    Ok(())
}
