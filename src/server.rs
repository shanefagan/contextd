//! Varlink server implementation and dynamic interface handling.

use crate::auth::{PeerInfo, set_current_peer};
use std::io::BufReader;
use std::os::unix::net::UnixListener;
use std::sync::Arc;
use threadpool::ThreadPool;
use varlink::{ConnectionHandler, VarlinkService};

/// A custom Varlink server implementation that captures peer credentials.
pub fn run_server(service: VarlinkService, address: &str) -> anyhow::Result<()> {
    let path = address.trim_start_matches("unix:");
    let listener = UnixListener::bind(path)?;
    // Use a small threadpool (4 threads) to prevent glibc malloc arena memory explosion (128 threads * 64MB arena = ~700MB RSS)
    let pool = ThreadPool::new(4);
    let service = Arc::new(service);

    log::debug!("Custom Varlink server listening on {}", path);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let service = Arc::clone(&service);
                let peer_info = PeerInfo::from_stream(&stream);

                pool.execute(move || {
                    set_current_peer(peer_info);
                    let mut reader = BufReader::new(&stream);
                    let mut writer = &stream;

                    if let Err(e) = service.handle(&mut reader, &mut writer, None) {
                        log::debug!("Connection closed: {}", e);
                    }
                    set_current_peer(None);
                });
            }
            Err(e) => {
                log::error!("Error accepting connection: {}", e);
            }
        }
    }

    Ok(())
}

/// A wrapper around a Varlink interface that allows for a dynamic description.
/// This is used to ensure the interface description is always in sync with the
/// source .varlink files, even when they are not available at compile time
/// in the standard way.
pub struct DynamicInterface {
    pub interface: Box<dyn varlink::Interface + Send + Sync>,
    pub description: &'static str,
}

impl varlink::Interface for DynamicInterface {
    fn get_name(&self) -> &'static str {
        self.interface.get_name()
    }

    fn get_description(&self) -> &'static str {
        self.description
    }

    fn call(&self, call: &mut varlink::Call) -> varlink::Result<()> {
        self.interface.call(call)
    }

    fn call_upgraded(
        &self,
        call: &mut varlink::Call,
        bufreader: &mut dyn std::io::BufRead,
    ) -> varlink::Result<Vec<u8>> {
        self.interface.call_upgraded(call, bufreader)
    }
}
