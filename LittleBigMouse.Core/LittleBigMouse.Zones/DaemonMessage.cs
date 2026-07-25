using System.Xml;
using System.Xml.Linq;

namespace LittleBigMouse.Zoning;

/// <summary>Strict parser for daemon event messages.</summary>
public readonly record struct DaemonMessage(LittleBigMouseEvent Event, string Payload)
{
    const int MaxMessageCharacters = 1024 * 1024;

    public static bool TryParse(string xml, out DaemonMessage message)
    {
        message = default;
        if (string.IsNullOrWhiteSpace(xml) || xml.Length > MaxMessageCharacters) return false;

        try
        {
            using var text = new StringReader(xml);
            using var reader = XmlReader.Create(text, new XmlReaderSettings
            {
                DtdProcessing = DtdProcessing.Prohibit,
                MaxCharactersInDocument = MaxMessageCharacters,
                XmlResolver = null,
            });
            var root = XDocument.Load(reader).Root;
            if (root?.Name.LocalName != "DaemonMessage") return false;

            var eventName = root.Element("Event")?.Value ?? root.Element("State")?.Value;
            var daemonEvent = eventName switch
            {
                "Running" => LittleBigMouseEvent.Running,
                "Stopped" => LittleBigMouseEvent.Stopped,
                "Paused" => LittleBigMouseEvent.Paused,
                "Dead" => LittleBigMouseEvent.Dead,
                "SettingChanged" or "SettingsChanged" => LittleBigMouseEvent.SettingsChanged,
                "DisplayChanged" => LittleBigMouseEvent.DisplayChanged,
                "DesktopChanged" => LittleBigMouseEvent.DesktopChanged,
                "FocusChanged" => LittleBigMouseEvent.FocusChanged,
                "Suspended" => LittleBigMouseEvent.Suspended,
                "Resumed" => LittleBigMouseEvent.Resumed,
                _ => (LittleBigMouseEvent?)null,
            };
            if (daemonEvent is null) return false;

            message = new DaemonMessage(daemonEvent.Value, root.Element("Payload")?.Value ?? "");
            return true;
        }
        catch (XmlException)
        {
            return false;
        }
    }
}
