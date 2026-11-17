//! OpenRGB SDK Bridge Daemon for contextd
//!
//! Listens for OpenRGB SDK clients on 127.0.0.1:6742 (override with
//! `OPENRGB_SERVER_PORT`) and presents one virtual controller. LED colors
//! sent to it become contextd's ambient color, via `SetLightingContext` on
//! `/run/contextd/private/contextd-rgb-control.socket` (override with
//! `CONTEXTD_CONTROL_SOCKET`).
//!
//! A real OpenRGB server uses the same default port, so run only one of
//! them on a port, or move this bridge to another.

mod protocol;

use contextd_client::{ControlClient, RgbaColor};
use log::{error, info, warn};
use protocol::*;
use std::env;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let port: u16 = env::var("OPENRGB_SERVER_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(OPENRGB_DEFAULT_PORT);

    let bind_addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(&bind_addr).await.map_err(|e| {
        anyhow::anyhow!(
            "cannot listen on {bind_addr}: {e}. Is OpenRGB's own SDK server running on this port? \
             Set OPENRGB_SERVER_PORT to use another."
        )
    })?;
    info!(
        "OpenRGB SDK Bridge listening on {} (forwarding to contextd)...",
        bind_addr
    );

    let control_socket = env::var("CONTEXTD_CONTROL_SOCKET").ok();
    let client = Arc::new(ControlClient::new(control_socket));

    // Latest color wins: clients may send updates faster than contextd
    // needs them, so one task forwards the newest value, in order.
    let (tx, rx) = watch::channel::<Option<Rgb>>(None);
    tokio::spawn(forward_colors(rx, client));
    let tx = Arc::new(tx);

    loop {
        match listener.accept().await {
            Ok((socket, addr)) => {
                info!("Accepted OpenRGB SDK client connection from {}", addr);
                let tx = Arc::clone(&tx);
                tokio::spawn(async move {
                    if let Err(e) = handle_client(socket, tx).await {
                        warn!("OpenRGB client connection error: {}", e);
                    }
                });
            }
            Err(e) => {
                error!("TCP accept error: {}", e);
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
    }
}

async fn forward_colors(mut rx: watch::Receiver<Option<Rgb>>, client: Arc<ControlClient>) {
    while rx.changed().await.is_ok() {
        let Some(rgb) = *rx.borrow_and_update() else {
            continue;
        };
        let client = Arc::clone(&client);
        let result = tokio::task::spawn_blocking(move || {
            client.set_main_color(RgbaColor::new(rgb[0], rgb[1], rgb[2], 255))
        })
        .await;
        match result {
            Ok(Ok(())) => {}
            Ok(Err(e)) => warn!("Failed to forward lighting context to contextd: {}", e),
            Err(e) => warn!("Forwarding task failed: {}", e),
        }
    }
}

async fn handle_client(
    mut socket: TcpStream,
    tx: Arc<watch::Sender<Option<Rgb>>>,
) -> anyhow::Result<()> {
    let mut protocol_version: u32 = 0;

    loop {
        let mut header_buf = [0u8; HEADER_SIZE];
        if socket.read_exact(&mut header_buf).await.is_err() {
            break; // Client disconnected
        }
        let header = match Header::read_from(&mut std::io::Cursor::new(&header_buf[..]))? {
            Some(h) => h,
            None => break,
        };

        if header.pkt_size > MAX_PACKET_SIZE {
            anyhow::bail!(
                "packet {} claims {} bytes (limit {}); closing connection",
                header.pkt_id,
                header.pkt_size,
                MAX_PACKET_SIZE
            );
        }
        let mut payload = vec![0u8; header.pkt_size as usize];
        socket.read_exact(&mut payload).await?;

        let reply = match header.pkt_id {
            PKT_REQUEST_PROTOCOL_VERSION => {
                let requested = payload
                    .get(0..4)
                    .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
                    .unwrap_or(0);
                protocol_version = requested.min(PROTOCOL_VERSION);
                info!("Client requested protocol version {requested}, using {protocol_version}");
                // We report our maximum; the client takes the minimum.
                Some(packet(
                    0,
                    PKT_REQUEST_PROTOCOL_VERSION,
                    &PROTOCOL_VERSION.to_le_bytes(),
                ))
            }
            PKT_REQUEST_CONTROLLER_COUNT => {
                Some(packet(0, PKT_REQUEST_CONTROLLER_COUNT, &1u32.to_le_bytes()))
            }
            PKT_REQUEST_CONTROLLER_DATA => {
                // Newer clients put the protocol version in the payload.
                let version = payload
                    .get(0..4)
                    .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
                    .unwrap_or(protocol_version)
                    .min(PROTOCOL_VERSION);
                let current = (*tx.borrow()).unwrap_or([255, 255, 255]);
                let data = build_controller_data(version, current);
                Some(packet(header.dev_id, PKT_REQUEST_CONTROLLER_DATA, &data))
            }
            PKT_REQUEST_PROFILE_LIST | PKT_REQUEST_PLUGIN_LIST => {
                Some(packet(0, header.pkt_id, &empty_list_reply()))
            }
            PKT_SET_CLIENT_NAME => {
                let name = String::from_utf8_lossy(&payload);
                info!(
                    "OpenRGB client registered name: {}",
                    name.trim_matches('\0')
                );
                None
            }
            PKT_UPDATE_LEDS | PKT_UPDATE_ZONE_LEDS | PKT_UPDATE_SINGLE_LED => {
                match parse_led_update(header.pkt_id, &payload) {
                    Ok(Some(colors)) => {
                        if let Some(rgb) = mean_color(&colors) {
                            tx.send_replace(Some(rgb));
                        }
                    }
                    Ok(None) => {}
                    Err(e) => warn!("Ignoring malformed packet {}: {}", header.pkt_id, e),
                }
                None
            }
            // Switching to our only (Direct) mode needs no action or reply.
            PKT_SET_CUSTOM_MODE => None,
            _ => None,
        };

        if let Some(reply) = reply {
            socket.write_all(&reply).await?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    async fn connected() -> (
        TcpStream,
        Arc<watch::Sender<Option<Rgb>>>,
        watch::Receiver<Option<Rgb>>,
    ) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (tx, rx) = watch::channel(None);
        let tx = Arc::new(tx);
        let server_tx = Arc::clone(&tx);
        tokio::spawn(async move {
            let (sock, _) = listener.accept().await.unwrap();
            let _ = handle_client(sock, server_tx).await;
        });
        (TcpStream::connect(addr).await.unwrap(), tx, rx)
    }

    async fn reply(s: &mut TcpStream) -> (Header, Vec<u8>) {
        let mut h = [0u8; HEADER_SIZE];
        s.read_exact(&mut h).await.unwrap();
        let h = Header::read_from(&mut std::io::Cursor::new(&h[..]))
            .unwrap()
            .unwrap();
        let mut p = vec![0u8; h.pkt_size as usize];
        s.read_exact(&mut p).await.unwrap();
        (h, p)
    }

    #[tokio::test]
    async fn full_client_session() {
        let (mut s, _tx, mut rx) = connected().await;

        s.write_all(&packet(
            0,
            PKT_REQUEST_PROTOCOL_VERSION,
            &9u32.to_le_bytes(),
        ))
        .await
        .unwrap();
        let (_, p) = reply(&mut s).await;
        assert_eq!(p, PROTOCOL_VERSION.to_le_bytes());

        s.write_all(&packet(0, PKT_REQUEST_CONTROLLER_DATA, &4u32.to_le_bytes()))
            .await
            .unwrap();
        let (h, p) = reply(&mut s).await;
        assert_eq!(h.pkt_id, PKT_REQUEST_CONTROLLER_DATA);
        assert_eq!(
            u32::from_le_bytes(p[0..4].try_into().unwrap()) as usize,
            p.len()
        );

        s.write_all(&packet(0, PKT_REQUEST_PLUGIN_LIST, &[]))
            .await
            .unwrap();
        assert_eq!(reply(&mut s).await.1, empty_list_reply());

        // UPDATELEDS with one pure-red LED, exactly as openrgb-python packs it.
        let leds = [10, 0, 0, 0, 1, 0, 255, 0, 0, 0];
        s.write_all(&packet(0, PKT_UPDATE_LEDS, &leds))
            .await
            .unwrap();
        rx.changed().await.unwrap();
        assert_eq!(*rx.borrow_and_update(), Some([255, 0, 0]));
    }

    #[tokio::test]
    async fn oversized_packet_closes_connection() {
        let (mut s, _tx, _rx) = connected().await;
        let mut hdr = Vec::new();
        Header {
            dev_id: 0,
            pkt_id: PKT_SET_CLIENT_NAME,
            pkt_size: 3 << 30,
        }
        .write_to(&mut hdr)
        .unwrap();
        s.write_all(&hdr).await.unwrap();
        let mut buf = [0u8; 1];
        assert_eq!(
            s.read(&mut buf).await.unwrap(),
            0,
            "server closed the connection"
        );
    }
}
