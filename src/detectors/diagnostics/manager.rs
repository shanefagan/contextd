use super::SystemDiagnostics;
use crate::contextd::Diagnostics;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct DiagnosticsManager {
    cached_diagnostics: Option<Diagnostics>,
}

impl DiagnosticsManager {
    pub fn new() -> Self {
        let mut manager = Self {
            cached_diagnostics: None,
        };
        manager.refresh();
        manager
    }

    pub fn refresh(&mut self) {
        log::info!("Refreshing system diagnostics cache...");
        let mut diag = SystemDiagnostics::get_diagnostics();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        diag.last_updated = now;
        self.cached_diagnostics = Some(diag);
    }

    pub fn get_diagnostics(&mut self) -> Diagnostics {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let needs_refresh = if let Some(ref diag) = self.cached_diagnostics {
            now - diag.last_updated > 300 // 5 minutes cache
        } else {
            true
        };

        if needs_refresh {
            self.refresh();
        }

        self.cached_diagnostics.clone().unwrap()
    }
}
