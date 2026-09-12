using LittleBigMouse.Zoning;

namespace LittleBigMouse.Ui.Avalonia.Tests;

/// <summary>
/// What an agent says, written out as it goes on the wire. Tests that need an agent to have
/// said something build it here rather than each holding its own idea of the shape — the
/// names are the protocol's, and a change to them has to land in one place.
/// </summary>
static class AgentFrames
{
    /// <summary>
    /// The agent's snapshot: the answer to <c>Subscribe</c>, and what a <c>State</c> event
    /// carries. Defaults to an agent that is there, hooked, and not running anything.
    /// </summary>
    public static string Snapshot(
        string engine = "Stopped",
        string? layoutId = null,
        bool hookConnected = true,
        bool previewing = false) => $$"""
        {"AgentVersion":"0.1.0","HookConnected":{{Bool(hookConnected)}},"Engine":"{{engine}}",
         "Suspended":false,"LayoutId":{{(layoutId is null ? "null" : $"\"{layoutId}\"")}},
         "Enabled":true,"Saved":true,"LoadAtStartup":false,"HideTrayIcon":false,
         "Previewing":{{Bool(previewing)}}}
        """;

    /// <summary>The agent's state changed.</summary>
    public static string State(
        string engine = "Stopped",
        string? layoutId = null,
        bool hookConnected = true,
        bool previewing = false)
        => $$"""{"Event":"State","State":{{Snapshot(engine, layoutId, hookConnected, previewing)}}}""";

    /// <summary>The hook said something, forwarded under the name the UI knows it by.</summary>
    public static string Hook(LittleBigMouseEvent hookEvent, string payload = "")
        => $$"""{"Event":"Hook","Hook":"{{hookEvent}}","Payload":"{{payload}}"}""";

    static string Bool(bool value) => value ? "true" : "false";
}
