//! A clipboard in memory, for tests: what the system's would do, driven by hand.
//!
//! [`Memory::copy`] is an application here copying, [`Memory::paste`] one reading; the
//! sharing drives the rest through the [`Clipboard`] it opened.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use lbm_net::wire::ClipFormat;
use tokio::sync::{mpsc, oneshot};

use super::{Clipboard, Command, Data, Event, Factory};

/// What the clipboard holds.
#[derive(Clone, Debug, PartialEq)]
pub enum Content {
    /// Copied here.
    Local(HashMap<ClipFormat, Vec<u8>>),
    /// Offered by the sharing, content not here.
    Offer {
        serial: u64,
        formats: Vec<ClipFormat>,
    },
}

#[derive(Default)]
struct Inner {
    content: Option<Content>,
    /// The sharing's ear while it holds the clipboard open.
    events: Option<mpsc::UnboundedSender<Event>>,
    /// How many times it was opened.
    opened: usize,
    /// The number of the last copy made here.
    copy: u64,
    /// How long a read takes, as a slow application or a large clipboard would.
    read_delay: std::time::Duration,
}

/// The clipboard, shared between the test and the sharing.
#[derive(Clone, Default)]
pub struct Memory(Arc<Mutex<Inner>>);

impl Memory {
    pub fn new() -> Memory {
        Memory::default()
    }

    /// Opens this clipboard, for the sharing.
    pub fn factory(&self) -> Factory {
        let memory = self.clone();
        Factory::new(move |events| {
            let mut inner = memory.0.lock().unwrap();
            inner.events = Some(events);
            inner.opened += 1;
            Ok(Box::new(Open(memory.clone())) as Box<dyn Clipboard>)
        })
    }

    /// Whether the sharing holds it open now.
    pub fn is_open(&self) -> bool {
        self.0.lock().unwrap().events.is_some()
    }

    /// How many times the sharing opened it.
    pub fn opened(&self) -> usize {
        self.0.lock().unwrap().opened
    }

    /// An application here copies `content`.
    pub fn copy(&self, content: Vec<(ClipFormat, Vec<u8>)>) {
        let formats = self.replace(content);
        let inner = self.0.lock().unwrap();
        if let Some(events) = &inner.events {
            let _ = events.send(Event::Changed {
                copy: inner.copy,
                formats,
            });
        }
    }

    /// An application here copies `content`, and the sharing has not heard of it yet:
    /// what a system clipboard looks like between a copy and its event.
    pub fn replace(&self, content: Vec<(ClipFormat, Vec<u8>)>) -> Vec<ClipFormat> {
        let content: HashMap<_, _> = content.into_iter().collect();
        let mut formats: Vec<ClipFormat> = content.keys().copied().collect();
        formats.sort_by_key(|format| *format as u8);
        let mut inner = self.0.lock().unwrap();
        inner.copy += 1;
        inner.content = Some(Content::Local(content));
        formats
    }

    /// Reads answer after `delay` from now on.
    pub fn slow_reads(&self, delay: std::time::Duration) {
        self.0.lock().unwrap().read_delay = delay;
    }

    pub fn content(&self) -> Option<Content> {
        self.0.lock().unwrap().content.clone()
    }

    /// An application here reads `format`: from what was copied here, or from the
    /// machine whose offer the clipboard holds.
    pub async fn paste(&self, format: ClipFormat) -> Data {
        let wanted = {
            let inner = self.0.lock().unwrap();
            match &inner.content {
                None => return Err("the clipboard is empty".into()),
                Some(Content::Local(content)) => {
                    return content
                        .get(&format)
                        .cloned()
                        .ok_or_else(|| "not in that format".into())
                }
                Some(Content::Offer { serial, formats }) => {
                    if !formats.contains(&format) {
                        return Err("not in that format".into());
                    }
                    let (reply, answer) = oneshot::channel();
                    let events = inner.events.clone().ok_or("nobody serves the offer")?;
                    events
                        .send(Event::Wanted {
                            serial: *serial,
                            format,
                            reply,
                        })
                        .map_err(|_| "nobody serves the offer")?;
                    answer
                }
            }
        };
        wanted
            .await
            .map_err(|_| "the offer was dropped".to_owned())?
    }
}

struct Open(Memory);

impl Clipboard for Open {
    fn command(&self, command: Command) {
        let mut inner = (self.0).0.lock().unwrap();
        match command {
            Command::Offer { serial, formats } => {
                inner.content = Some(Content::Offer { serial, formats })
            }
            Command::Withdraw { serial } => {
                if matches!(&inner.content, Some(Content::Offer { serial: s, .. }) if *s == serial)
                {
                    inner.content = None;
                }
            }
            Command::Read {
                copy,
                format,
                limit,
                reply,
            } => {
                let data = match &inner.content {
                    Some(Content::Local(_)) if copy != inner.copy => Err(super::CHANGED.to_owned()),
                    Some(Content::Local(content)) => match content.get(&format) {
                        Some(data) if data.len() > limit => Err(super::too_large(limit)),
                        Some(data) => Ok(data.clone()),
                        None => Err("not in that format".to_owned()),
                    },
                    _ => Err("nothing copied here".to_owned()),
                };
                let delay = inner.read_delay;
                if delay.is_zero() {
                    let _ = reply.send(data);
                } else {
                    std::thread::spawn(move || {
                        std::thread::sleep(delay);
                        let _ = reply.send(data);
                    });
                }
            }
        }
    }
}

impl Drop for Open {
    fn drop(&mut self) {
        let mut inner = (self.0).0.lock().unwrap();
        inner.events = None;
        if matches!(inner.content, Some(Content::Offer { .. })) {
            inner.content = None;
        }
    }
}
