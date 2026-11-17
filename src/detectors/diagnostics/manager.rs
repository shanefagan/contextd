use super::SystemDiagnostics;
use crate::contextd::Diagnostics;
use std::time::{SystemTime, UNIX_EPOCH};

/// Caches diagnostics, which are expensive to gather. Nothing is collected
/// until the first request.
#[derive(Default)]
pub struct DiagnosticsManager {
    cached: Option<Diagnostics>,
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

impl DiagnosticsManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_diagnostics(&mut self) -> Diagnostics {
        let ttl = crate::config::get().ttls.diagnostics as i64;
        let now = now();
        match &self.cached {
            Some(d) if now - d.last_updated <= ttl && now >= d.last_updated => d.clone(),
            _ => {
                log::info!("Refreshing system diagnostics cache...");
                let mut diag = SystemDiagnostics::get_diagnostics();
                diag.last_updated = now;
                self.cached = Some(diag.clone());
                diag
            }
        }
    }
}
