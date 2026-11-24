//! The X11 hider: XFixes hides the cursor of the whole screen while this client asks,
//! and the server shows it again by itself if the client goes away.

use std::os::fd::{AsRawFd, RawFd};

use x11rb::connection::Connection;
use x11rb::protocol::xfixes::ConnectionExt as _;
use x11rb::rust_connection::RustConnection;

use super::Missing;

const LOG: &str = "[LittleBigMouse.Hook] cursor hider (X11)";

pub(super) struct Hider {
    connection: RustConnection,
    root: u32,
    hidden: bool,
}

impl Hider {
    pub(super) fn connect() -> Result<Hider, Missing> {
        let (connection, screen) = RustConnection::connect(None)
            .map_err(|e| Missing::Unreachable(format!("cannot reach the X server ({e})")))?;
        // XFixes 4 is the version that has HideCursor; the request also tells the
        // server which version this client speaks, which it must hear first.
        let version = connection
            .xfixes_query_version(4, 0)
            .ok()
            .and_then(|cookie| cookie.reply().ok());
        if version.is_none_or(|v| v.major_version < 4) {
            return Err(Missing::Unsupported("the X server has no XFixes 4".into()));
        }
        let root = connection.setup().roots[screen].root;
        eprintln!("{LOG}: connected");
        Ok(Hider {
            connection,
            root,
            hidden: false,
        })
    }

    pub(super) fn reconcile(&mut self, hide: bool) -> Result<(), String> {
        if hide == self.hidden {
            return Ok(());
        }
        let sent = if hide {
            self.connection.xfixes_hide_cursor(self.root).map(|_| ())
        } else {
            self.connection.xfixes_show_cursor(self.root).map(|_| ())
        };
        sent.map_err(|e| e.to_string())?;
        self.connection.flush().map_err(|e| e.to_string())?;
        self.hidden = hide;
        eprintln!("{LOG}: cursor {}", if hide { "hidden" } else { "shown" });
        Ok(())
    }

    pub(super) fn wait(&mut self, wake: RawFd) -> Result<(), String> {
        let mut fds = [
            libc::pollfd {
                fd: self.connection.stream().as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: wake,
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        super::poll(&mut fds);
        // Nothing is selected: whatever comes is an error reply or noise, read so the
        // connection does not fill up, and a closed connection is an error here.
        while self
            .connection
            .poll_for_event()
            .map_err(|e| e.to_string())?
            .is_some()
        {}
        Ok(())
    }
}
