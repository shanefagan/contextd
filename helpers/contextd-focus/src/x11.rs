//! X11 / XWayland: `_NET_ACTIVE_WINDOW` on the root window.

use crate::FocusSource;
use anyhow::Context;
use x11rb::connection::{Connection, RequestConnection};
use x11rb::protocol::Event;
use x11rb::protocol::res::{self, ClientIdMask, ClientIdSpec, ConnectionExt as _};
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ChangeWindowAttributesAux, ConnectionExt as _, EventMask, Window,
};
use x11rb::rust_connection::RustConnection;

pub struct X11 {
    conn: RustConnection,
    root: Window,
    net_active_window: Atom,
    net_wm_pid: Atom,
    has_res: bool,
}

impl X11 {
    pub fn connect() -> anyhow::Result<Self> {
        let (conn, screen) = x11rb::connect(None).context("cannot connect to the X server")?;
        let root = conn.setup().roots[screen].root;
        let net_active_window = conn
            .intern_atom(false, b"_NET_ACTIVE_WINDOW")?
            .reply()?
            .atom;
        let net_wm_pid = conn.intern_atom(false, b"_NET_WM_PID")?.reply()?.atom;
        conn.change_window_attributes(
            root,
            &ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE),
        )?;
        let has_res = conn
            .extension_information(res::X11_EXTENSION_NAME)?
            .is_some();
        if !has_res {
            log::warn!("X-Resource extension missing; falling back to _NET_WM_PID");
        }
        conn.flush()?;
        Ok(Self {
            conn,
            root,
            net_active_window,
            net_wm_pid,
            has_res,
        })
    }

    fn active_window(&self) -> anyhow::Result<Option<Window>> {
        let reply = self
            .conn
            .get_property(
                false,
                self.root,
                self.net_active_window,
                AtomEnum::ANY,
                0,
                1,
            )?
            .reply()?;
        Ok(reply
            .value32()
            .and_then(|mut v| v.next())
            .filter(|&w| w != 0))
    }

    /// The PID of the client that created `window`, as the X server sees
    /// it. Unlike `_NET_WM_PID` this is right for sandboxed clients.
    fn pid_from_res(&self, window: Window) -> Option<u32> {
        let spec = ClientIdSpec {
            client: window,
            mask: ClientIdMask::LOCAL_CLIENT_PID,
        };
        let reply = self.conn.res_query_client_ids(&[spec]).ok()?.reply().ok()?;
        reply
            .ids
            .iter()
            .find(|id| id.spec.mask.contains(ClientIdMask::LOCAL_CLIENT_PID))
            .and_then(|id| id.value.first().copied())
            .filter(|&pid| pid > 0)
    }

    fn pid_from_property(&self, window: Window) -> Option<u32> {
        let reply = self
            .conn
            .get_property(false, window, self.net_wm_pid, AtomEnum::CARDINAL, 0, 1)
            .ok()?
            .reply()
            .ok()?;
        reply
            .value32()
            .and_then(|mut v| v.next())
            .filter(|&pid| pid > 0)
    }
}

impl FocusSource for X11 {
    fn current(&mut self) -> anyhow::Result<Option<u32>> {
        let Some(window) = self.active_window()? else {
            return Ok(None);
        };
        let from_res = if self.has_res {
            self.pid_from_res(window)
        } else {
            None
        };
        Ok(from_res.or_else(|| self.pid_from_property(window)))
    }

    fn wait(&mut self) -> anyhow::Result<()> {
        loop {
            match self.conn.wait_for_event()? {
                Event::PropertyNotify(e) if e.atom == self.net_active_window => return Ok(()),
                _ => {}
            }
        }
    }
}
