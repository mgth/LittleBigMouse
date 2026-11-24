//! This machine's clipboard, as the multi-PC mode shares it (docs/multi-pc-etat.md,
//! "Presse-papiers partagé").
//!
//! A [`Clipboard`] is the system's clipboard seen through one narrow window: it says
//! when something new was copied here, and in which of the shared formats it can be read
//! ([`Event::Changed`]); it reads that on demand ([`Command::Read`]); and it holds what
//! another machine offers without its content, asking for the content only when an
//! application here reads it ([`Command::Offer`], then [`Event::Wanted`]). What it put
//! there itself it never reports as a change: that is what keeps two machines from
//! sending one copy back and forth for ever.
//!
//! Each system's clipboard runs on a thread of its own and is driven by events only:
//! `ext-data-control-v1` under Wayland ([`wayland`]), a clipboard format listener and
//! delayed rendering under Windows ([`windows`]). [`memory`] stands in for both in tests.
//! None is opened unless the user turned the sharing on and a machine that shares is
//! linked.

use std::fmt;
use std::sync::Arc;

use lbm_net::wire::ClipFormat;
use tokio::sync::{mpsc, oneshot};

pub mod convert;
pub mod memory;
#[cfg(target_os = "linux")]
mod wayland;
#[cfg(windows)]
mod windows;

/// Some content, or why there is none.
pub type Data = Result<Vec<u8>, String>;

/// Why a read of a copy no longer on the clipboard is refused.
pub const CHANGED: &str = "the clipboard changed since";

/// Why a content over `limit` bytes does not cross, in words for the user.
pub fn too_large(limit: usize) -> String {
    format!(
        "the clipboard is larger than the bound of {} MiB",
        limit.div_ceil(1024 * 1024)
    )
}

/// What the sharing asks of the clipboard.
#[derive(Debug)]
pub enum Command {
    /// Take the clipboard over with what another machine holds, in `formats`, without
    /// its content: each format is asked for ([`Event::Wanted`]) the first time an
    /// application here reads it. `serial` names this offer in what follows.
    Offer {
        serial: u64,
        formats: Vec<ClipFormat>,
    },
    /// The offer `serial` cannot be served any more (its machine left): let go of the
    /// clipboard if it still holds it, so that a paste finds nothing rather than waits.
    Withdraw { serial: u64 },
    /// Read `format` of the copy numbered `copy` ([`Event::Changed`]), no more than
    /// `limit` bytes: past it the answer is an error, and what was read is thrown away.
    /// Refused when the clipboard holds another copy by now, or the same one marked as
    /// not to be shared since: what crosses is only ever what was announced.
    Read {
        copy: u64,
        format: ClipFormat,
        limit: usize,
        reply: oneshot::Sender<Data>,
    },
}

/// What the clipboard tells the sharing.
#[derive(Debug)]
pub enum Event {
    /// Something was copied here, readable in `formats` (empty: nothing that is shared —
    /// a format of no use elsewhere, a secret, or an empty clipboard). Never said of an
    /// [`Command::Offer`]. `copy` numbers it for [`Command::Read`].
    Changed { copy: u64, formats: Vec<ClipFormat> },
    /// An application here reads `format` of the offer `serial`: the content goes to
    /// `reply`, which it waits for.
    Wanted {
        serial: u64,
        format: ClipFormat,
        reply: oneshot::Sender<Data>,
    },
    /// The clipboard cannot go on (the compositor dropped the connection, the window
    /// went): it says nothing more.
    Failed(String),
}

/// One system clipboard, open. Dropping it lets go of the clipboard and stops its thread.
pub trait Clipboard: Send {
    fn command(&self, command: Command);
}

type Open =
    dyn Fn(mpsc::UnboundedSender<Event>) -> Result<Box<dyn Clipboard>, String> + Send + Sync;

/// How the sharing opens the clipboard: the system's ([`Factory::system`]), or a test's.
#[derive(Clone)]
pub struct Factory(Arc<Open>);

impl Factory {
    pub fn new(
        open: impl Fn(mpsc::UnboundedSender<Event>) -> Result<Box<dyn Clipboard>, String>
            + Send
            + Sync
            + 'static,
    ) -> Factory {
        Factory(Arc::new(open))
    }

    /// Opens the clipboard; what it says goes to `events`. An error says why there is no
    /// clipboard to share here, in words for the user.
    pub fn open(&self, events: mpsc::UnboundedSender<Event>) -> Result<Box<dyn Clipboard>, String> {
        (self.0)(events)
    }

    /// This system's clipboard.
    pub fn system() -> Factory {
        Factory::new(system)
    }
}

impl fmt::Debug for Factory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("clipboard::Factory")
    }
}

#[cfg(target_os = "linux")]
fn system(events: mpsc::UnboundedSender<Event>) -> Result<Box<dyn Clipboard>, String> {
    wayland::open(events)
}

#[cfg(windows)]
fn system(events: mpsc::UnboundedSender<Event>) -> Result<Box<dyn Clipboard>, String> {
    windows::open(events)
}

#[cfg(not(any(target_os = "linux", windows)))]
fn system(_: mpsc::UnboundedSender<Event>) -> Result<Box<dyn Clipboard>, String> {
    Err("the clipboard is not shared on this system".to_owned())
}
