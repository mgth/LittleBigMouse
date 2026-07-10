#nullable enable
using LittleBigMouse.DisplayLayout;
using LittleBigMouse.DisplayLayout.Monitors;

namespace LittleBigMouse.Plugins;

/// <summary>
/// Platform seam: persists the layout model and the app options. Windows stores them in the
/// registry (HKCU\SOFTWARE\Mgth\LittleBigMouse), Linux in JSON files under
/// ~/.config/LittleBigMouse. Shared UI code depends only on this interface; the entry points
/// mirror the historical extension methods 1:1.
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

    /// <summary>Persist only the Enabled flag (engine on/off) and align the autostart.</summary>
    bool SaveEnabled(IMonitorsLayout layout);

    /// <summary>
    /// Persist the app-level options immediately. They take effect at the next app start
    /// (or right away for the UI), never through the engine start flow, so waiting for the
    /// save button would just lose them.
    /// </summary>
    void SaveLive(ILayoutOptions options);
}
