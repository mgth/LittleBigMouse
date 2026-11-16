#nullable enable
using LittleBigMouse.DisplayLayout;
using LittleBigMouse.DisplayLayout.Monitors;

namespace LittleBigMouse.Plugins;

/// <summary>
/// Platform seam: persists the layout model and the app options, in the agent's JSON store
/// on both systems (v6, D2): ~/.config/LittleBigMouse on Linux,
/// %LOCALAPPDATA%\Mgth\LittleBigMouse on Windows, where the 5.x registry is read only until
/// the agent has imported it. In v6 the window only reads through it: the agent is the
/// only writer. Shared UI code depends only on this interface; the entry points mirror the
/// historical extension methods 1:1.
/// </summary>
public interface ILayoutPersistence
{
    /// <summary>
    /// True while options are being (re)loaded: live-save subscriptions must not react to
    /// the setters fired by a load (re-scheduling the startup task mid-load would drop the
    /// elevation, and echoing values back is wasted writes at best).
    /// </summary>
    bool IsLoading { get; }

    /// <summary>Restore saved geometry, borders and options into a freshly built layout.</summary>
    void Load(MonitorsLayout layout);

    /// <summary>Persist the whole layout: options, monitors, models.</summary>
    bool Save(MonitorsLayout layout);

    /// <summary>Persist only the Enabled flag (engine on/off).</summary>
    bool SaveEnabled(IMonitorsLayout layout);

    /// <summary>
    /// Persist the app-level options immediately. They take effect at the next app start
    /// (or right away for the UI), never through the engine start flow, so waiting for the
    /// save button would just lose them.
    /// </summary>
    void SaveLive(ILayoutOptions options);
}
