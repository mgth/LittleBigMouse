//! Wire protocol: parse incoming `CommandMessage`s and build outgoing
//! `DaemonMessage`s.
//!
//! Port of the parsing in `LittleBigMouseDaemon::ReceiveMessage` and the fixed
//! event strings in `SendState`/`DisplayChanged`/`FocusChanged`. The C# side only
//! substring-matches the outgoing events (`LittleBigMouseClientService`), so the
//! event *strings* are what matters; the incoming parse must be exact.

use roxmltree::{Document, Node};

/// A command received from the UI (`<CommandMessage Command="..."/>`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Listen,
    /// The extracted `<ZonesLayout>...</ZonesLayout>` XML (empty if absent).
    Load(String),
    LoadFromFile(String),
    Run,
    Stop,
    State,
    Quit,
    Unknown(String),
}

/// Parse one received line into zero or more commands.
///
/// Accepts a bare `<CommandMessage .../>` or a `<Messages>` container wrapping
/// several of them (both handled by the C++ `ReceiveMessage`). Returns an empty
/// vec on blank input or malformed XML — matching the C++ tolerance.
pub fn parse(line: &str) -> Vec<Command> {
    let text = line.trim();
    if text.is_empty() {
        return Vec::new();
    }
    match Document::parse(text) {
        Ok(doc) => collect(doc.root_element()),
        Err(_) => Vec::new(),
    }
}

fn collect(node: Node) -> Vec<Command> {
    match node.tag_name().name() {
        "CommandMessage" => command_from(node).into_iter().collect(),
        "Messages" => node
            .children()
            .filter(Node::is_element)
            .flat_map(collect)
            .collect(),
        _ => Vec::new(),
    }
}

fn command_from(node: Node) -> Option<Command> {
    let command = node.attribute("Command")?;
    Some(match command {
        "Listen" => Command::Listen,
        "Load" => Command::Load(zones_layout_xml(node)),
        "LoadFromFile" => Command::LoadFromFile(payload_string(node)),
        "Run" => Command::Run,
        "Stop" => Command::Stop,
        "State" => Command::State,
        "Quit" => Command::Quit,
        other => Command::Unknown(other.to_string()),
    })
}

/// Slice out the `<Payload><ZonesLayout>...</ZonesLayout></Payload>` subtree's
/// source XML so it can be parsed on its own. Returns "" if absent.
fn zones_layout_xml(node: Node) -> String {
    node.children()
        .find(|c| c.has_tag_name("Payload"))
        .and_then(|p| p.children().find(|c| c.has_tag_name("ZonesLayout")))
        .map(|z| z.document().input_text()[z.range()].to_string())
        .unwrap_or_default()
}

/// Read the `Payload` for `LoadFromFile`, whether serialized as an attribute
/// (`Payload="..."`) or a child element (`<Payload>...</Payload>`).
fn payload_string(node: Node) -> String {
    if let Some(attr) = node.attribute("Payload") {
        return attr.to_string();
    }
    node.children()
        .find(|c| c.has_tag_name("Payload"))
        .and_then(|p| p.text())
        .unwrap_or("")
        .to_string()
}

// --- Outgoing daemon events (exact strings, each `\n`-terminated) ------------

pub const RUNNING: &str = "<DaemonMessage><Event>Running</Event></DaemonMessage>\n";
pub const STOPPED: &str = "<DaemonMessage><Event>Stopped</Event></DaemonMessage>\n";
pub const PAUSED: &str = "<DaemonMessage><Event>Paused</Event></DaemonMessage>\n";
pub const DISPLAY_CHANGED: &str = "<DaemonMessage><Event>DisplayChanged</Event></DaemonMessage>\n";
pub const SETTING_CHANGED: &str = "<DaemonMessage><Event>SettingChanged</Event></DaemonMessage>\n";
pub const DESKTOP_CHANGED: &str = "<DaemonMessage><Event>DesktopChanged</Event></DaemonMessage>\n";
pub const SUSPENDED: &str = "<DaemonMessage><Event>Suspended</Event></DaemonMessage>\n";
pub const RESUMED: &str = "<DaemonMessage><Event>Resumed</Event></DaemonMessage>\n";

/// Build a `FocusChanged` event carrying the foreground process path.
pub fn focus_changed(path: &str) -> String {
    format!(
        "<DaemonMessage><Event>FocusChanged</Event><Payload>{}</Payload></DaemonMessage>\n",
        escape_xml(path)
    )
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_listen() {
        assert_eq!(
            parse(r#"<CommandMessage Command="Listen" Payload=""/>"#),
            vec![Command::Listen]
        );
    }

    #[test]
    fn tolerates_trailing_carriage_return() {
        // .NET StreamWriter.WriteLine emits `\r\n`; framing splits on `\n`,
        // leaving a trailing `\r` that must not break parsing.
        assert_eq!(
            parse("<CommandMessage Command=\"Run\" Payload=\"\"/>\r"),
            vec![Command::Run]
        );
    }

    #[test]
    fn parses_load_extracts_zones_layout_xml() {
        let msg = r#"<CommandMessage Command="Load"><Payload><ZonesLayout MaxTravelDistance="200"/></Payload></CommandMessage>"#;
        assert_eq!(
            parse(msg),
            vec![Command::Load(
                r#"<ZonesLayout MaxTravelDistance="200"/>"#.to_string()
            )]
        );
    }

    #[test]
    fn parses_messages_container() {
        let msg = concat!(
            "<Messages>",
            r#"<CommandMessage Command="Load"/>"#,
            r#"<CommandMessage Command="Run"/>"#,
            "</Messages>"
        );
        // Load with no Payload -> empty ZonesLayout XML.
        assert_eq!(parse(msg), vec![Command::Load(String::new()), Command::Run]);
    }

    #[test]
    fn blank_and_malformed_yield_nothing() {
        assert!(parse("   ").is_empty());
        assert!(parse("not xml <<<").is_empty());
    }

    #[test]
    fn focus_payload_is_well_formed_xml() {
        let message = focus_changed(r#"C:\A&B\<game>.exe"#);
        let document = Document::parse(&message).unwrap();
        let payload = document
            .descendants()
            .find(|node| node.has_tag_name("Payload"))
            .unwrap();
        assert_eq!(payload.text(), Some(r#"C:\A&B\<game>.exe"#));
    }
}
