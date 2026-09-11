//! The client side of the protocol: the commands a controller sends the daemon, and
//! the events it reads back.
//!
//! Until v6 the only client was the C# UI (`LittleBigMouse.Zones/CommandMessage.cs`
//! and `DaemonMessage.cs`); the v6 agent is the next one. Commands are built byte for
//! byte as the C# `CommandMessage.Serialize` writes them — the `ui-to-daemon` goldens
//! of `wire-contract/` are C#'s output — and events are read as strictly as
//! `DaemonMessage.TryParse`: an event this version does not know is rejected, never
//! mapped onto a known one, so a newer daemon degrades to "state unchanged" instead of
//! to a wrong state.

use roxmltree::Document;

//==================//
// Commands         //
//==================//

/// A command that carries nothing (`Run`, `Stop`, `Quit`, `State`, `Listen`, `Probe`),
/// as C#'s `ZoneSerializer` writes a `CommandMessage` whose payload is null.
fn bare(command: &str) -> String {
    format!(r#"<CommandMessage Command="{command}" Payload=""></CommandMessage>"#)
}

/// `Run`: hook the loaded layout.
pub fn run() -> String {
    bare("Run")
}

/// `Stop`: unhook.
pub fn stop() -> String {
    bare("Stop")
}

/// `Quit`: unhook and exit.
pub fn quit() -> String {
    bare("Quit")
}

/// `State`: have the daemon say whether it runs.
pub fn state() -> String {
    bare("State")
}

/// `Listen`: subscribe to the daemon's events — written as the C# `LocalIpcClient`
/// writes it on connecting (not through `CommandMessage`, hence the other spelling).
pub fn listen() -> String {
    r#"<CommandMessage Command="Listen" Payload=""/>"#.to_owned()
}

/// `Probe`: sweep the loaded layout's edges and report (`Probed`).
pub fn probe() -> String {
    bare("Probe")
}

/// `Load`: hand the daemon a layout. `zones_layout` is a serialized `<ZonesLayout>`
/// document (`lbm_layout::zoning::ZonesLayout::serialize`).
pub fn load(zones_layout: &str) -> String {
    format!(r#"<CommandMessage Command="Load"><Payload>{zones_layout}</Payload></CommandMessage>"#)
}

/// One frame holding several commands, as the C# client sends every batch
/// (`SendMessagesAsync`): `Load` and `Run` together keep the hook up across a
/// re-apply instead of unhooking and hooking again.
pub fn messages(commands: &[String]) -> String {
    format!("<Messages>{}</Messages>", commands.concat())
}

/// `Shortcut`: adopt this panic shortcut now. The text is escaped as C#'s
/// `SecurityElement.Escape` does.
pub fn shortcut(combination: &str) -> String {
    format!(
        r#"<CommandMessage Command="Shortcut" Payload="{}"/>"#,
        security_element_escape(combination)
    )
}

/// `System.Security.SecurityElement.Escape`.
fn security_element_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            '&' => out.push_str("&amp;"),
            c => out.push(c),
        }
    }
    out
}

//==================//
// Events           //
//==================//

/// What the daemon reports: C#'s `LittleBigMouseEvent`, less `Connected` (a client
/// state, never on the wire).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DaemonEvent {
    /// The input hook is installed.
    Running,
    /// The input hook is down.
    Stopped,
    /// Hooked but standing aside (an excluded application has the focus).
    Paused,
    Dead,
    /// A system setting changed (`SettingChanged`, or the legacy `SettingsChanged`).
    SettingsChanged,
    DisplayChanged,
    DesktopChanged,
    /// Payload: the foreground process path.
    FocusChanged,
    /// The display turned off; the daemon unhooked itself.
    Suspended,
    /// The display is back.
    Resumed,
    /// A `Load` was parsed; payload: a summary.
    Loaded,
    LoadFailed,
    /// Payload: a `<ProbeReport>` document.
    Probed,
    /// The panic shortcut ran.
    Rescued,
    /// Payload: the panic shortcut that could not be registered.
    ShortcutUnavailable,
}

/// One event read from the daemon: `DaemonMessage`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DaemonMessage {
    pub event: DaemonEvent,
    /// The `<Payload>` text, `""` when there is none.
    pub payload: String,
}

/// C#'s `DaemonMessage.MaxMessageCharacters`: the daemon's frame limit.
const MAX_MESSAGE_CHARACTERS: usize = 1024 * 1024;

/// C# `DaemonMessage.TryParse`: a `<DaemonMessage>` whose `<Event>` (or, from older
/// daemons, `<State>`) names an event this version knows. `None` for anything else:
/// blank or oversized text, XML that does not parse (a DTD included), another root, an
/// unknown event.
pub fn parse_event(xml: &str) -> Option<DaemonMessage> {
    if xml.trim().is_empty() || xml.encode_utf16().count() > MAX_MESSAGE_CHARACTERS {
        return None;
    }
    let doc = Document::parse(xml).ok()?;
    let root = doc.root_element();
    if root.tag_name().name() != "DaemonMessage" {
        return None;
    }

    // XElement.Value: all the text inside the element.
    let value = |name: &str| {
        root.children()
            .find(|c| {
                c.is_element() && c.tag_name().name() == name && c.tag_name().namespace().is_none()
            })
            .map(|e| {
                e.descendants()
                    .filter(|d| d.is_text())
                    .filter_map(|d| d.text())
                    .collect::<String>()
            })
    };

    let event = match value("Event").or_else(|| value("State"))?.as_str() {
        "Running" => DaemonEvent::Running,
        "Stopped" => DaemonEvent::Stopped,
        "Paused" => DaemonEvent::Paused,
        "Dead" => DaemonEvent::Dead,
        "SettingChanged" | "SettingsChanged" => DaemonEvent::SettingsChanged,
        "DisplayChanged" => DaemonEvent::DisplayChanged,
        "DesktopChanged" => DaemonEvent::DesktopChanged,
        "FocusChanged" => DaemonEvent::FocusChanged,
        "Suspended" => DaemonEvent::Suspended,
        "Resumed" => DaemonEvent::Resumed,
        "Loaded" => DaemonEvent::Loaded,
        "LoadFailed" => DaemonEvent::LoadFailed,
        "Probed" => DaemonEvent::Probed,
        "Rescued" => DaemonEvent::Rescued,
        "ShortcutUnavailable" => DaemonEvent::ShortcutUnavailable,
        _ => return None,
    };
    Some(DaemonMessage {
        event,
        payload: value("Payload").unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_are_written_as_csharp_writes_them() {
        assert_eq!(
            state(),
            r#"<CommandMessage Command="State" Payload=""></CommandMessage>"#
        );
        assert_eq!(
            shortcut(r#"<A & 'B' "C">"#),
            r#"<CommandMessage Command="Shortcut" Payload="&lt;A &amp; &apos;B&apos; &quot;C&quot;&gt;"/>"#
        );
    }

    #[test]
    fn a_batch_is_one_messages_frame_the_hook_splits_back() {
        let frame = messages(&[load("<ZonesLayout/>"), run()]);
        assert_eq!(
            frame,
            r#"<Messages><CommandMessage Command="Load"><Payload><ZonesLayout/></Payload></CommandMessage><CommandMessage Command="Run" Payload=""></CommandMessage></Messages>"#
        );
        assert_eq!(
            crate::protocol::parse(&frame),
            [
                crate::protocol::Command::Load("<ZonesLayout/>".into()),
                crate::protocol::Command::Run
            ]
        );
    }

    #[test]
    fn events_are_read_as_strictly_as_csharp() {
        let message = |xml: &str| parse_event(xml);
        assert_eq!(
            message("<DaemonMessage><Event>Running</Event></DaemonMessage>"),
            Some(DaemonMessage {
                event: DaemonEvent::Running,
                payload: String::new()
            })
        );
        // Legacy spellings.
        assert_eq!(
            message("<DaemonMessage><State>SettingsChanged</State></DaemonMessage>")
                .map(|m| m.event),
            Some(DaemonEvent::SettingsChanged)
        );
        // Unknown events, other roots, blank text, broken XML, DTDs: rejected.
        for xml in [
            "<DaemonMessage><Event>Teleported</Event></DaemonMessage>",
            "<DaemonMessage><Event>running</Event></DaemonMessage>",
            "<DaemonMessage/>",
            "<CommandMessage Command=\"Run\"/>",
            "  ",
            "<DaemonMessage><Event>Running</Event>",
            "<!DOCTYPE d [<!ENTITY e \"Running\">]><DaemonMessage><Event>&e;</Event></DaemonMessage>",
        ] {
            assert_eq!(message(xml), None, "{xml}");
        }
        assert_eq!(parse_event(&"x".repeat(MAX_MESSAGE_CHARACTERS + 1)), None);
    }
}
