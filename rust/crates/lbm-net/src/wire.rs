//! What crosses a link: frames, and the control messages some of them carry.
//!
//! A frame is a four-byte little-endian length, one kind byte, then a body of that
//! length (the length counts the body only, not the kind byte). Kind 0 is a control
//! message, one JSON object; kind 1 is one batch of `lbm_ipc::input` events, which the
//! agents relay without reading. Both kinds share one connection so that a click and
//! the `Leave` that follows it cannot overtake each other.
//!
//! Kind 2 is a piece of a bulk transfer ([`Bulk`]): what a control message is too large
//! to carry, the content of a clipboard. It is cut small and always leaves after
//! whatever control or input is waiting (`link`), so a large transfer never holds a
//! click up by more than one piece. Only a peer that announced
//! [`CLIPBOARD_CAPABILITY`] is sent one: an older peer reads an unknown kind as a
//! broken link.

use std::io;

use lbm_ipc::peer::{PeerDisplay, PeerWallpaper};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// The version of this protocol, stated in `Hello`. Two nodes that disagree refuse
/// each other rather than guess.
pub const PROTOCOL: u32 = 1;

/// The kind byte of a control frame.
pub const CONTROL: u8 = 0;
/// The kind byte of an input frame.
pub const INPUT: u8 = 1;
/// The kind byte of a piece of a bulk transfer.
pub const BULK: u8 = 2;

/// A control message is a few screens' worth of JSON at most: the bound of the local
/// IPC frames serves here too.
pub const MAX_CONTROL_SIZE: usize = lbm_ipc::framing::MAX_FRAME_SIZE;
/// An input frame carries exactly what the hook's input endpoint does.
pub const MAX_INPUT_SIZE: usize = lbm_ipc::input::MAX_BATCH_SIZE;
/// The data of one piece of a bulk transfer. Small on purpose: an input frame queued
/// behind a piece waits for it to be written, and 16 KiB is a fraction of a millisecond
/// on a wired network.
pub const BULK_PIECE: usize = 16 * 1024;
/// A bulk frame: the transfer's number, a flags byte, then the data.
const BULK_HEADER: usize = 9;
const MAX_BULK_SIZE: usize = BULK_HEADER + BULK_PIECE;

/// The capability of a node that shares its clipboard (docs/multi-pc-etat.md,
/// "Presse-papiers partagé"): it understands the `Clipboard*` messages and bulk frames.
/// Neither is ever sent to a peer that did not announce it.
pub const CLIPBOARD_CAPABILITY: &str = "Clipboard";

/// The capability, in `Hello`, of a node that draws on its screens the overlays its
/// master asks for ([`Control::Overlay`]). A master sends them only to a remote that
/// announces it: an older one would merely ignore the unknown type, but its user would
/// then see rulers on some screens and not others with nothing to say why.
pub const OVERLAY_CAPABILITY: &str = "RemoteOverlay";

/// Which end of the input stream a node is on this link.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Role {
    /// Owns the mouse and keyboard and the layout; sends input.
    Master,
    /// Reports its screens and injects what it receives.
    Remote,
}

/// The first message on every link, from both sides, before either knows whether it
/// trusts the other.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Hello {
    pub protocol: u32,
    /// The machine's name as the user knows it, shown when pairing.
    #[serde(default)]
    pub name: String,
    /// `linux`, `windows`, … as `std::env::consts::OS` says.
    #[serde(default)]
    pub os: String,
    /// The product version, for the user and for diagnostics; never for decisions.
    #[serde(default)]
    pub version: String,
    pub role: Role,
    /// What this node can do beyond the protocol's baseline. Unknown names are
    /// ignored, which is how the protocol grows without a new version.
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// A remote's Windows service, which takes the input stream itself where its
    /// agent cannot inject (docs/multi-pc-service-windows.md). Absent everywhere else;
    /// a node that does not know the field ignores it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secure_input: Option<SecureInput>,
}

/// Where a remote's service listens for the input stream, and the fingerprint of the
/// certificate it answers with — not the agent's: the service has an identity of its
/// own, which the master checks when it connects. Told on the authenticated link with
/// the remote's agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct SecureInput {
    pub port: u16,
    pub machine_id: String,
}

impl Hello {
    /// This node's `Hello` at the current protocol, on this OS.
    pub fn new(name: impl Into<String>, version: impl Into<String>, role: Role) -> Hello {
        Hello {
            protocol: PROTOCOL,
            name: name.into(),
            os: std::env::consts::OS.to_owned(),
            version: version.into(),
            role,
            capabilities: Vec::new(),
            secure_input: None,
        }
    }
}

/// A control message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "Type",
    rename_all = "PascalCase",
    rename_all_fields = "PascalCase"
)]
pub enum Control {
    Hello(Hello),
    /// This side trusts the other: the answer to its `Hello` once the user (or the
    /// pairing store) said yes.
    Paired,
    /// This side does not trust the other, or cannot talk to it. The connection ends.
    Refused {
        #[serde(default)]
        reason: String,
    },
    /// Remote → master: the remote's screens, at connection and after every change.
    /// A higher revision replaces a lower one.
    Displays {
        revision: u64,
        #[serde(default)]
        displays: Vec<PeerDisplay>,
    },
    /// Remote → master: what each of the remote's screens shows, drawn small, at link-up
    /// and after its display changes. A higher revision replaces a lower one. Added after
    /// protocol 1 without a new version: an older master sees an unknown type.
    Wallpapers {
        revision: u64,
        #[serde(default)]
        wallpapers: Vec<PeerWallpaper>,
    },
    /// Either way: write VCP 0x60 = `value` on that display. `request` numbers it, for
    /// the answer (`InputSwitched`) to say which request it answers: a retry of the same
    /// value must not be settled by the answer to the attempt before it. Added after
    /// protocol 1: absent from an older master, ignored by an older remote.
    SelectInput {
        display: String,
        value: u16,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        request: Option<u64>,
    },
    /// Remote → master: the remote ended the session of the master's input stream
    /// although the link lives on (its hook could not keep up, and what it held was
    /// released). What the master sends next is ignored there until an `Enter`, so the
    /// master brings its pointer home and starts afresh. Added after protocol 1 without
    /// a new version: an older master sees an unknown type.
    InputReset,
    /// Remote → master: how the `SelectInput` for `display` and `value` went on the
    /// remote, with that request's `request`: `error` says why it could not, absent when
    /// the monitor shows that input now. What lets the master's user see a switch that
    /// failed on the other machine. Added after protocol 1 without a new version: an
    /// older master sees an unknown type, and an older remote sends none.
    InputSwitched {
        display: String,
        value: u16,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        request: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
    /// Master → remote: draw these overlays on the remote's screens, in place of any
    /// drawn before; an empty list takes them all down. `bands` are
    /// `lbm_overlay::scene::Band`s in the remote's own desktop pixels, read from the
    /// screens it reported (`Displays`): the remote checks each against its screens as
    /// they are now and refuses the whole list otherwise. A higher revision replaces a
    /// lower one. The remote takes them down by itself when the link ends. Sent only to a
    /// remote announcing [`OVERLAY_CAPABILITY`]. Added after protocol 1 without a new
    /// version.
    ///
    /// `interactive`: the bands take the pointer and the keyboard, and what is done on
    /// them comes back as [`Control::OverlayInput`]; otherwise they let them through. A
    /// remote that does not know the field draws them passive, and never answers with
    /// inputs.
    Overlay {
        revision: u64,
        #[serde(default)]
        bands: Vec<serde_json::Value>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        interactive: bool,
    },
    /// Remote → master: a gesture on one of the interactive overlays of `revision`
    /// (or an earlier one: a drag outlives the revisions it causes), in order. `input`
    /// is an `lbm_overlay::input::Input`, its band as the remote shows it. Only the master
    /// whose overlays are shown is sent them, and it acts on them itself — the remote
    /// edits nothing. Added after [`Control::Overlay`], without a new version.
    OverlayInput {
        revision: u64,
        input: serde_json::Value,
    },
    /// Remote → master: the overlays of `revision` are not shown, and why — refused, or
    /// taken down because a screen changed under them. Added with [`Control::Overlay`].
    OverlayFailed {
        revision: u64,
        #[serde(default)]
        error: String,
    },
    /// Remote → master: Windows drops what the remote injects from now on (`block`
    /// set), or no longer does (`None`) — see [`lbm_ipc::peer::InjectionBlock`]. Sent
    /// only to a master that announced [`lbm_ipc::peer::INJECTION_BLOCKED_CAPABILITY`].
    /// A block said while that master drives the remote ends its crossing: the pointer
    /// comes home rather than stay frozen there. Added after protocol 1 without a new
    /// version.
    InjectionBlocked {
        #[serde(default)]
        block: Option<lbm_ipc::peer::InjectionBlock>,
    },
    /// Either way, between nodes that both announced [`CLIPBOARD_CAPABILITY`]: the
    /// sender's clipboard now holds something new, available in `formats`, numbered
    /// `serial` on the sender. Nothing of the content: it crosses only when an
    /// application on the other side reads it (`ClipboardRequest`). Empty `formats` says
    /// the sender holds nothing it shares any more.
    ///
    /// With [`ClipFormat::Files`], `files` lists what was copied, folders walked: what
    /// the other side needs to ask for each file and to know the size of all before it
    /// asks.
    ///
    /// `seen` is the last `serial` the sender heard from the receiver: an offer whose
    /// `seen` is older than the receiver's own last announcement crossed it, two copies
    /// made at once, which both sides settle the same way (the lower machine id wins).
    /// `receiving` says whether the sender takes the receiver's offers at all: when it
    /// does not, there is nothing to settle, and its copy is taken.
    ClipboardOffer {
        serial: u64,
        #[serde(default)]
        formats: Vec<ClipFormat>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        files: Vec<ClipFile>,
        #[serde(default, skip_serializing_if = "is_zero")]
        seen: u64,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        receiving: bool,
    },
    /// The answer to a `ClipboardOffer`: `format` of what was offered under `serial`,
    /// please, as the bulk transfer numbered `transfer` (numbered by the asker). Answered
    /// by bulk frames of that transfer, the last one marked, or by `ClipboardAbort`.
    ///
    /// For [`ClipFormat::Files`], one file at a time and in ranges: `item` is its index
    /// in the offer's `files`, and the transfer carries `length` bytes from `offset` (to
    /// the file's end without `length`). The asker asks for the next range once it has
    /// written the last: that is all the flow control a large file needs.
    ClipboardRequest {
        serial: u64,
        format: ClipFormat,
        transfer: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        item: Option<u32>,
        #[serde(default, skip_serializing_if = "is_zero")]
        offset: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        length: Option<u64>,
    },
    /// Either way: the transfer numbered `transfer` ends without its data — the sender
    /// cannot give it (`reason` says why), or the asker no longer wants it.
    ClipboardAbort {
        transfer: u64,
        #[serde(default)]
        reason: String,
    },
    Ping,
    Pong,
    /// The sender is closing the link on purpose.
    Bye {
        #[serde(default)]
        reason: String,
    },
    /// A message type this build does not know: tolerated, so that a newer peer can
    /// say more without being cut off.
    #[serde(other)]
    Unknown,
}

/// What a clipboard holds, as it crosses: one representation per kind of content,
/// whatever each system calls it (docs/multi-pc-etat.md, "Formats, par palier").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ClipFormat {
    /// UTF-8 text, lines ended by `\n`.
    Text,
    /// A PNG picture.
    Png,
    /// Files and folders, listed in the offer and carried one file at a time.
    Files,
    /// A format this build does not know: a newer peer offers more, and what this one
    /// knows of the offer is still worth taking.
    #[serde(other)]
    Unknown,
}

/// One entry of a [`ClipFormat::Files`] offer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ClipFile {
    /// Where it goes, relative to wherever it is pasted: its name, after the names of
    /// the folders copied around it, separated by `/` whatever the system. A folder
    /// comes before what is in it.
    pub path: String,
    /// Its size in bytes; 0 for a folder.
    #[serde(default)]
    pub size: u64,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub folder: bool,
}

fn is_zero(value: &u64) -> bool {
    *value == 0
}

/// One piece of a bulk transfer.
#[derive(Debug, Clone, PartialEq)]
pub struct Bulk {
    /// The transfer it belongs to, as its `ClipboardRequest` numbered it.
    pub transfer: u64,
    /// The transfer's last piece: what came before it is the whole of the data.
    pub last: bool,
    pub data: Vec<u8>,
}

/// One frame, decoded as far as this side reads it.
#[derive(Debug, Clone, PartialEq)]
pub enum Frame {
    Control(Control),
    /// An input batch, still encoded.
    Input(Vec<u8>),
    Bulk(Bulk),
}

/// A control message as the bytes of a whole frame, ready to write.
pub fn encode_control(message: &Control) -> io::Result<Vec<u8>> {
    let body = serde_json::to_vec(message).map_err(io::Error::other)?;
    encode(CONTROL, &body)
}

/// An input batch as the bytes of a whole frame, ready to write.
pub fn encode_input(batch: &[u8]) -> io::Result<Vec<u8>> {
    encode(INPUT, batch)
}

/// A piece of a bulk transfer as the bytes of a whole frame, ready to write. `data` is
/// at most [`BULK_PIECE`] bytes.
pub fn encode_bulk(piece: &Bulk) -> io::Result<Vec<u8>> {
    let mut body = Vec::with_capacity(BULK_HEADER + piece.data.len());
    body.extend_from_slice(&piece.transfer.to_le_bytes());
    body.push(u8::from(piece.last));
    body.extend_from_slice(&piece.data);
    encode(BULK, &body)
}

fn encode(kind: u8, body: &[u8]) -> io::Result<Vec<u8>> {
    if body.len() > limit(kind)? {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "frame body exceeds the limit of its kind",
        ));
    }
    let mut frame = Vec::with_capacity(5 + body.len());
    frame.extend_from_slice(&(body.len() as u32).to_le_bytes());
    frame.push(kind);
    frame.extend_from_slice(body);
    Ok(frame)
}

fn limit(kind: u8) -> io::Result<usize> {
    match kind {
        CONTROL => Ok(MAX_CONTROL_SIZE),
        INPUT => Ok(MAX_INPUT_SIZE),
        BULK => Ok(MAX_BULK_SIZE),
        other => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unknown frame kind {other}"),
        )),
    }
}

/// Writes one encoded frame in a single write: one task owns the writer, but a frame
/// that left in pieces would still be a frame that can be cut in two by a failure.
pub async fn write_frame<W: AsyncWrite + Unpin>(writer: &mut W, frame: &[u8]) -> io::Result<()> {
    writer.write_all(frame).await?;
    writer.flush().await
}

/// Reads one frame. Anything that is not a frame of ours — an unknown kind, a length
/// over its kind's bound (checked before allocating), a control body that is not a
/// JSON message — is an error, and the link it came on is not worth keeping: there is
/// no telling where the next frame would start.
///
/// Not cancel-safe: a frame read halfway is lost. Run it in a task of its own.
pub async fn read_frame<R: AsyncRead + Unpin>(reader: &mut R) -> io::Result<Frame> {
    let length = reader.read_u32_le().await? as usize;
    let kind = reader.read_u8().await?;
    if length > limit(kind)? {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "frame body exceeds the limit of its kind",
        ));
    }
    let mut body = vec![0u8; length];
    reader.read_exact(&mut body).await?;
    if kind == INPUT {
        return Ok(Frame::Input(body));
    }
    if kind == BULK {
        if body.len() < BULK_HEADER {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "bulk frame shorter than its header",
            ));
        }
        let transfer = u64::from_le_bytes(body[..8].try_into().expect("eight bytes"));
        let last = body[8] & 1 != 0;
        body.drain(..BULK_HEADER);
        return Ok(Frame::Bulk(Bulk {
            transfer,
            last,
            data: body,
        }));
    }
    serde_json::from_slice(&body)
        .map(Frame::Control)
        .map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("control frame: {error}"),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn round_trip(frame: &[u8]) -> io::Result<Frame> {
        read_frame(&mut &frame[..]).await
    }

    #[test]
    fn a_remotes_service_is_told_in_its_hello_and_absent_otherwise() {
        let plain = Hello::new("tower", "6.0.0", Role::Remote);
        let json = serde_json::to_value(&plain).unwrap();
        assert!(json.get("SecureInput").is_none(), "{json}");

        let offered = Hello {
            secure_input: Some(SecureInput {
                port: 24818,
                machine_id: "ab".repeat(32),
            }),
            ..plain.clone()
        };
        let json = serde_json::to_value(&offered).unwrap();
        assert_eq!(json["SecureInput"]["Port"], 24818);
        assert_eq!(json["SecureInput"]["MachineId"], "ab".repeat(32));
        assert_eq!(serde_json::from_value::<Hello>(json).unwrap(), offered);

        // From a node that does not know the field.
        let older: Hello = serde_json::from_value(serde_json::json!({
            "Protocol": PROTOCOL, "Role": "Remote"
        }))
        .unwrap();
        assert_eq!(older.secure_input, None);
    }

    #[tokio::test]
    async fn a_control_message_crosses_as_pascal_case_json() {
        let hello = Control::Hello(Hello {
            capabilities: vec!["MultiPc".into()],
            ..Hello::new("desk", "6.0.0", Role::Master)
        });
        let frame = encode_control(&hello).unwrap();
        assert_eq!(frame[4], CONTROL);
        let json: serde_json::Value = serde_json::from_slice(&frame[5..]).unwrap();
        assert_eq!(json["Type"], "Hello");
        assert_eq!(json["Protocol"], PROTOCOL);
        assert_eq!(json["Role"], "Master");
        assert_eq!(json["Capabilities"][0], "MultiPc");
        assert_eq!(round_trip(&frame).await.unwrap(), Frame::Control(hello));

        for message in [
            Control::Paired,
            Control::Refused {
                reason: "no".into(),
            },
            Control::Displays {
                revision: 3,
                displays: vec![],
            },
            Control::SelectInput {
                display: "DP-1".into(),
                value: 0x0f,
                request: Some(7),
            },
            Control::InputReset,
            Control::InputSwitched {
                display: "DP-1".into(),
                value: 0x0f,
                request: None,
                error: None,
            },
            Control::InputSwitched {
                display: "DP-1".into(),
                value: 0x0f,
                request: Some(7),
                error: Some("DDC/CI write failed".into()),
            },
            Control::Ping,
            Control::Pong,
            Control::Bye {
                reason: "quit".into(),
            },
        ] {
            let frame = encode_control(&message).unwrap();
            assert_eq!(round_trip(&frame).await.unwrap(), Frame::Control(message));
        }
        let select = encode_control(&Control::SelectInput {
            display: "DP-1".into(),
            value: 17,
            request: None,
        })
        .unwrap();
        assert_eq!(
            std::str::from_utf8(&select[5..]).unwrap(),
            r#"{"Type":"SelectInput","Display":"DP-1","Value":17}"#
        );
    }

    #[tokio::test]
    async fn wallpapers_cross_as_a_picture_or_a_colour_per_display() {
        let message = Control::Wallpapers {
            revision: 4,
            wallpapers: vec![
                PeerWallpaper {
                    display: "eDP-1".into(),
                    image: Some("/9j/4AAQ".into()),
                    color: None,
                },
                PeerWallpaper {
                    display: "HDMI-1".into(),
                    image: None,
                    color: Some("#203040".into()),
                },
            ],
        };
        let frame = encode_control(&message).unwrap();
        assert_eq!(
            std::str::from_utf8(&frame[5..]).unwrap(),
            concat!(
                r#"{"Type":"Wallpapers","Revision":4,"Wallpapers":["#,
                r#"{"Display":"eDP-1","Image":"/9j/4AAQ"},"#,
                r##"{"Display":"HDMI-1","Color":"#203040"}]}"##
            )
        );
        assert_eq!(round_trip(&frame).await.unwrap(), Frame::Control(message));
        // The bound of a whole message of wallpapers (512 KiB of pictures) fits a control
        // frame once in base64, with room for the JSON around it.
        const { assert!(512 * 1024 * 4 / 3 + 64 * 1024 < MAX_CONTROL_SIZE) };
    }

    #[test]
    fn an_input_reset_is_a_bare_type_an_older_peer_reads_as_unknown() {
        let reset = encode_control(&Control::InputReset).unwrap();
        assert_eq!(
            std::str::from_utf8(&reset[5..]).unwrap(),
            r#"{"Type":"InputReset"}"#
        );
    }

    #[test]
    fn an_input_switch_says_why_only_when_it_failed() {
        let done = encode_control(&Control::InputSwitched {
            display: "DP-1".into(),
            value: 17,
            request: None,
            error: None,
        })
        .unwrap();
        assert_eq!(
            std::str::from_utf8(&done[5..]).unwrap(),
            r#"{"Type":"InputSwitched","Display":"DP-1","Value":17}"#
        );
        let failed = encode_control(&Control::InputSwitched {
            display: "DP-1".into(),
            value: 17,
            request: Some(3),
            error: Some("no answer".into()),
        })
        .unwrap();
        assert_eq!(
            std::str::from_utf8(&failed[5..]).unwrap(),
            r#"{"Type":"InputSwitched","Display":"DP-1","Value":17,"Request":3,"Error":"no answer"}"#
        );
        // What an older master sends reads, with no request to echo.
        let older: Control =
            serde_json::from_str(r#"{"Type":"SelectInput","Display":"DP-1","Value":17}"#).unwrap();
        assert_eq!(
            older,
            Control::SelectInput {
                display: "DP-1".into(),
                value: 17,
                request: None
            }
        );
    }

    #[test]
    fn an_injection_block_names_the_process_and_its_end_is_a_null() {
        let blocked = Control::InjectionBlocked {
            block: Some(lbm_ipc::peer::InjectionBlock::elevated("osk.exe")),
        };
        let bytes = encode_control(&blocked).unwrap();
        let text = std::str::from_utf8(&bytes[5..]).unwrap();
        assert_eq!(
            text,
            r#"{"Type":"InjectionBlocked","Block":{"Process":"osk.exe","Cause":"Integrity"}}"#
        );
        assert_eq!(serde_json::from_str::<Control>(text).unwrap(), blocked);
        // Without a cause, the first one; one this version does not know, unknown.
        let causeless: Control =
            serde_json::from_str(r#"{"Type":"InjectionBlocked","Block":{"Process":"osk.exe"}}"#)
                .unwrap();
        assert_eq!(causeless, blocked);
        let unknown: Control = serde_json::from_str(
            r#"{"Type":"InjectionBlocked","Block":{"Process":"","Cause":"Hypervisor"}}"#,
        )
        .unwrap();
        assert_eq!(
            unknown,
            Control::InjectionBlocked {
                block: Some(lbm_ipc::peer::InjectionBlock::unknown())
            }
        );
        let unblocked: Control =
            serde_json::from_str(r#"{"Type":"InjectionBlocked","Block":null}"#).unwrap();
        assert_eq!(unblocked, Control::InjectionBlocked { block: None });
        // A newer remote may say more about the block.
        let newer: Control = serde_json::from_str(
            r#"{"Type":"InjectionBlocked","Block":{"Process":"osk.exe","Level":12288}}"#,
        )
        .unwrap();
        assert_eq!(newer, blocked);
    }

    #[tokio::test]
    async fn overlays_cross_as_a_revision_and_their_bands_unread() {
        let band = serde_json::json!({"Monitor": "B", "Side": "Top"});
        let show = Control::Overlay {
            revision: 2,
            bands: vec![band.clone()],
            interactive: false,
        };
        let frame = encode_control(&show).unwrap();
        assert_eq!(
            std::str::from_utf8(&frame[5..]).unwrap(),
            r#"{"Type":"Overlay","Revision":2,"Bands":[{"Monitor":"B","Side":"Top"}]}"#
        );
        assert_eq!(round_trip(&frame).await.unwrap(), Frame::Control(show));
        // Taking them down is the same message with no bands, which may be left out.
        let hide: Control = serde_json::from_str(r#"{"Type":"Overlay","Revision":3}"#).unwrap();
        assert_eq!(
            hide,
            Control::Overlay {
                revision: 3,
                bands: vec![],
                interactive: false,
            }
        );
        // Interactive overlays say so; the gestures come back with their revision.
        let dragged = Control::Overlay {
            revision: 4,
            bands: vec![band.clone()],
            interactive: true,
        };
        let frame = encode_control(&dragged).unwrap();
        assert_eq!(
            std::str::from_utf8(&frame[5..]).unwrap(),
            r#"{"Type":"Overlay","Revision":4,"Bands":[{"Monitor":"B","Side":"Top"}],"Interactive":true}"#
        );
        assert_eq!(round_trip(&frame).await.unwrap(), Frame::Control(dragged));
        let input = Control::OverlayInput {
            revision: 4,
            input: serde_json::json!({"kind": "Press", "at": 12.0}),
        };
        let frame = encode_control(&input).unwrap();
        assert_eq!(
            std::str::from_utf8(&frame[5..]).unwrap(),
            r#"{"Type":"OverlayInput","Revision":4,"Input":{"kind":"Press","at":12.0}}"#
        );
        assert_eq!(round_trip(&frame).await.unwrap(), Frame::Control(input));
        let failed = Control::OverlayFailed {
            revision: 2,
            error: "Screen bounds changed.".into(),
        };
        let frame = encode_control(&failed).unwrap();
        assert_eq!(
            std::str::from_utf8(&frame[5..]).unwrap(),
            r#"{"Type":"OverlayFailed","Revision":2,"Error":"Screen bounds changed."}"#
        );
        assert_eq!(round_trip(&frame).await.unwrap(), Frame::Control(failed));
    }

    #[tokio::test]
    async fn an_unknown_message_type_is_tolerated() {
        let body = br#"{"Type":"Clipboard","Text":"later"}"#;
        let mut frame = (body.len() as u32).to_le_bytes().to_vec();
        frame.push(CONTROL);
        frame.extend_from_slice(body);
        assert_eq!(
            round_trip(&frame).await.unwrap(),
            Frame::Control(Control::Unknown)
        );
    }

    #[tokio::test]
    async fn the_clipboard_messages_cross_and_an_unknown_format_is_kept_as_such() {
        for message in [
            Control::ClipboardOffer {
                serial: 4,
                formats: vec![ClipFormat::Text, ClipFormat::Png],
                files: vec![],
                seen: 3,
                receiving: true,
            },
            Control::ClipboardOffer {
                serial: 5,
                formats: vec![],
                files: vec![],
                seen: 0,
                receiving: false,
            },
            Control::ClipboardOffer {
                serial: 6,
                formats: vec![ClipFormat::Files],
                files: vec![
                    ClipFile {
                        path: "photos".into(),
                        size: 0,
                        folder: true,
                    },
                    ClipFile {
                        path: "photos/été.jpg".into(),
                        size: 3_000_000,
                        folder: false,
                    },
                ],
                seen: 0,
                receiving: false,
            },
            Control::ClipboardRequest {
                serial: 4,
                format: ClipFormat::Png,
                transfer: 9,
                item: None,
                offset: 0,
                length: None,
            },
            Control::ClipboardRequest {
                serial: 6,
                format: ClipFormat::Files,
                transfer: 10,
                item: Some(1),
                offset: 4 << 20,
                length: Some(4 << 20),
            },
            Control::ClipboardAbort {
                transfer: 9,
                reason: "too large".into(),
            },
        ] {
            let frame = encode_control(&message).unwrap();
            assert_eq!(round_trip(&frame).await.unwrap(), Frame::Control(message));
        }
        let newer: Control = serde_json::from_str(
            r#"{"Type":"ClipboardOffer","Serial":1,"Formats":["Text","Rtf"]}"#,
        )
        .unwrap();
        assert_eq!(
            newer,
            Control::ClipboardOffer {
                serial: 1,
                formats: vec![ClipFormat::Text, ClipFormat::Unknown],
                files: vec![],
                seen: 0,
                receiving: false,
            }
        );
    }

    #[tokio::test]
    async fn a_bulk_piece_crosses_with_its_transfer_and_its_end() {
        for piece in [
            Bulk {
                transfer: u64::MAX - 1,
                last: false,
                data: vec![7; BULK_PIECE],
            },
            Bulk {
                transfer: 3,
                last: true,
                data: vec![],
            },
        ] {
            let frame = encode_bulk(&piece).unwrap();
            assert_eq!(frame[4], BULK);
            assert_eq!(round_trip(&frame).await.unwrap(), Frame::Bulk(piece));
        }
        let too_large = Bulk {
            transfer: 1,
            last: true,
            data: vec![0; BULK_PIECE + 1],
        };
        assert!(encode_bulk(&too_large).is_err());
        // A body too short for its header is not a frame of ours.
        let mut short = 3u32.to_le_bytes().to_vec();
        short.push(BULK);
        short.extend_from_slice(&[1, 2, 3]);
        assert!(round_trip(&short).await.is_err());
    }

    #[tokio::test]
    async fn an_input_batch_crosses_unread() {
        // Not a valid batch: the network does not care.
        let frame = encode_input(&[200, 1, 2]).unwrap();
        assert_eq!(frame[4], INPUT);
        assert_eq!(
            round_trip(&frame).await.unwrap(),
            Frame::Input(vec![200, 1, 2])
        );
    }

    #[tokio::test]
    async fn what_is_not_a_frame_of_ours_is_an_error() {
        let oversized_input = {
            let mut f = (MAX_INPUT_SIZE as u32 + 1).to_le_bytes().to_vec();
            f.push(INPUT);
            f
        };
        let oversized_control = {
            let mut f = (MAX_CONTROL_SIZE as u32 + 1).to_le_bytes().to_vec();
            f.push(CONTROL);
            f
        };
        let unknown_kind = [0, 0, 0, 0, 7];
        let not_json = {
            let mut f = 3u32.to_le_bytes().to_vec();
            f.push(CONTROL);
            f.extend_from_slice(b"abc");
            f
        };
        let no_type = {
            let body = br#"{"Reason":"x"}"#;
            let mut f = (body.len() as u32).to_le_bytes().to_vec();
            f.push(CONTROL);
            f.extend_from_slice(body);
            f
        };
        for frame in [
            &oversized_input[..],
            &oversized_control,
            &unknown_kind,
            &not_json,
            &no_type,
        ] {
            let error = round_trip(frame).await.unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::InvalidData, "{frame:?}");
        }
        assert!(encode_input(&vec![0; MAX_INPUT_SIZE + 1]).is_err());
    }
}
