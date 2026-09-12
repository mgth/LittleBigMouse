using DynamicData;
using HLab.Geo;
using LittleBigMouse.DisplayLayout;
using LittleBigMouse.DisplayLayout.Dimensions;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Plugins;

namespace LittleBigMouse.Ui.Avalonia.Tests;

/// <summary>
/// The world <c>MainService</c> and its collaborators talk to, reduced to what can be asserted
/// on: a store that records writes, and a display configuration that can be moved by hand.
/// <para>
/// The agent is not among them: it is reached through <see cref="AgentClient"/>, which tests
/// drive by handing it the frames an agent would have sent (see <see cref="AgentFrames"/>).
/// </para>
/// </summary>
static class MainServiceFakes
{
    /// <summary>
    /// A layout that is real — <c>ComputeZones</c> walks it for every Start — but minimal: one
    /// monitor, one source, attached.
    /// </summary>
    public static MonitorsLayout NewLayout(
        ILayoutOptions options, bool enabled = true, LayoutSource source = LayoutSource.System)
    {
        var layout = new MonitorsLayout(options) { Id = "TESTMON1", Source = source };
        layout.Options.Enabled = enabled;

        var model = new PhysicalMonitorModel("TST1234");
        model.PhysicalSize.Width = 600;
        model.PhysicalSize.Height = 340;

        var monitor = new PhysicalMonitor("MON1", layout, model);
        var displaySource = new DisplaySource("SRC1") { AttachedToDesktop = true };
        displaySource.InPixel.Set(new Rect(new Point(0, 0), new Size(1920, 1080)));

        var physicalSource = new PhysicalSource("DEV1", monitor, displaySource);
        monitor.ActiveSource = physicalSource;
        monitor.Sources.Add(physicalSource);

        layout.AddOrUpdatePhysicalMonitor(monitor);
        layout.AddOrUpdatePhysicalSource(physicalSource);
        return layout;
    }
}

/// <summary>The store, reduced to what was written to it.</summary>
sealed class FakePersistence : ILayoutPersistence
{
    public bool IsLoading { get; set; }
    public int EnabledWrites { get; private set; }
    public bool? LastEnabledWritten { get; private set; }
    public int LiveOptionWrites { get; private set; }

    /// <summary>Full layout saves — the geometry, not just the engine's on/off.</summary>
    public int LayoutWrites { get; private set; }

    public void Load(MonitorsLayout layout) { }

    public bool Save(MonitorsLayout layout)
    {
        LayoutWrites++;
        return true;
    }

    public bool SaveEnabled(IMonitorsLayout layout)
    {
        EnabledWrites++;
        LastEnabledWritten = layout.Options.Enabled;
        return true;
    }

    public void SaveLive(ILayoutOptions options) => LiveOptionWrites++;
}

/// <summary>
/// The platform's view of the displays: a signature the test moves by hand, a layout it hands
/// out on request, and the two events the platform raises on its own.
/// </summary>
sealed class FakeLayoutFactory(Func<MonitorsLayout> create) : ILayoutFactory
{
    public string Signature { get; set; } = "one-monitor";
    public int Creations { get; private set; }
    public int WallpaperUpdates { get; private set; }

    public MonitorsLayout Create()
    {
        Creations++;
        return create();
    }

    public string DisplaySignature() => Signature;

    public event EventHandler? DisplayChanged;
    public event EventHandler? WallpaperChanged;

    public void RaiseDisplayChanged() => DisplayChanged?.Invoke(this, EventArgs.Empty);
    public void RaiseWallpaperChanged() => WallpaperChanged?.Invoke(this, EventArgs.Empty);

    /// <summary>True while anything is still subscribed — what a released subscription must undo.</summary>
    public bool HasDisplaySubscribers => DisplayChanged is not null;
    public bool HasWallpaperSubscribers => WallpaperChanged is not null;

    public void UpdateWallpaper(MonitorsLayout layout) => WallpaperUpdates++;
}
