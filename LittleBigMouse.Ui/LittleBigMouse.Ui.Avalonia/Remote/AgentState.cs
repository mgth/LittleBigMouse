#nullable enable
using System.Text.Json.Serialization;

namespace LittleBigMouse.Ui.Avalonia.Remote;

/// <summary>
/// What the agent says of itself — its `Snapshot`, and the `State` event it sends a
/// subscriber whenever that snapshot changes (v6 API, see <c>docs/v6-agent.md</c>).
/// </summary>
/// <param name="Engine">"Running", "Stopped", "Paused" or "Dead", as the hook reports it.</param>
/// <param name="LayoutId">
/// The id of the layout the agent holds. The UI builds its own model from its own
/// enumeration, so a different id means the two are not looking at the same desktop.
/// </param>
/// <param name="Previewing">A frontend's live preview is what the hook runs.</param>
public sealed record AgentState(
    string AgentVersion,
    bool HookConnected,
    string Engine,
    bool Suspended,
    string? LayoutId,
    bool? Enabled,
    bool? Saved,
    bool? LoadAtStartup,
    bool? HideTrayIcon,
    bool Previewing)
{
    /// <summary>The engine's state as the UI's events name it.</summary>
    [JsonIgnore]
    public LittleBigMouse.Zoning.LittleBigMouseEvent EngineEvent => Engine switch
    {
        "Running" => Zoning.LittleBigMouseEvent.Running,
        "Stopped" => Zoning.LittleBigMouseEvent.Stopped,
        "Paused" => Zoning.LittleBigMouseEvent.Paused,
        _ => Zoning.LittleBigMouseEvent.Dead,
    };
}
