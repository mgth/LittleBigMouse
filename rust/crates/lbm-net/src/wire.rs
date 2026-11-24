//! What crosses a link: frames, and the control messages some of them carry.
//!
//! A frame is a four-byte little-endian length, one kind byte, then a body of that
//! length (the length counts the body only, not the kind byte). Kind 0 is a control
//! message, one JSON object; kind 1 is one batch of `lbm_ipc::input` events, which the
//! agents relay without reading. Both kinds share one connection so that a click and
//! the `Leave` that follows it cannot overtake each other.

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

/// A control message is a few screens' worth of JSON at most: the bound of the local
/// IPC frames serves here too.
pub const MAX_CONTROL_SIZE: usize = lbm_ipc::framing::MAX_FRAME_SIZE;
/// An input frame carries exactly what the hook's input endpoint does.
pub const MAX_INPUT_SIZE: usize = lbm_ipc::input::MAX_BATCH_SIZE;

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
    /// Either way: write VCP 0x60 = `value` on that display.
    SelectInput {
        display: String,
        value: u16,
    },
    /// Remote → master: the remote ended the session of the master's input stream
    /// although the link lives on (its hook could not keep up, and what it held was
    /// released). What the master sends next is ignored there until an `Enter`, so the
    /// master brings its pointer home and starts afresh. Added after protocol 1 without
    /// a new version: an older master sees an unknown type.
    InputReset,
    /// Remote → master: how the `SelectInput` for `display` and `value` went on the
    /// remote: `error` says why it could not, absent when the monitor shows that input
    /// now. What lets the master's user see a switch that failed on the other machine.
    /// Added after protocol 1 without a new version: an older master sees an unknown
    /// type, and an older remote sends none.
    InputSwitched {
        display: String,
        value: u16,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<String>,
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

/// One frame, decoded as far as this side reads it.
#[derive(Debug, Clone, PartialEq)]
pub enum Frame {
    Control(Control),
    /// An input batch, still encoded.
    Input(Vec<u8>),
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
            },
            Control::InputReset,
            Control::InputSwitched {
                display: "DP-1".into(),
                value: 0x0f,
                error: None,
            },
            Control::InputSwitched {
                display: "DP-1".into(),
                value: 0x0f,
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
            error: Some("no answer".into()),
        })
        .unwrap();
        assert_eq!(
            std::str::from_utf8(&failed[5..]).unwrap(),
            r#"{"Type":"InputSwitched","Display":"DP-1","Value":17,"Error":"no answer"}"#
        );
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
