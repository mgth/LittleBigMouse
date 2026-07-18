#nullable enable
using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Threading;
using DynamicData;
using LittleBigMouse.DisplayLayout.Dimensions;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.DisplayLayout.Monitors.Extensions;
using LittleBigMouse.Plugins;

namespace LittleBigMouse.Platform.Linux;

/// <summary>
/// Linux implementation of <see cref="ILayoutFactory"/>. Discovery goes through the best
/// available <see cref="ILinuxMonitorSource"/> — KScreen on KDE (compositor's own logical
/// geometry, per-output scale, priority), XRandR anywhere else — enriched with EDID identity
/// and physical size read from /sys/class/drm. Since no daemon reports display changes on
/// Linux yet, the factory polls the sysfs plug signature and raises
/// <see cref="DisplayChanged"/> itself; MainService's settle/idempotence logic does the rest.
/// </summary>
public class LinuxLayoutFactory : ILayoutFactory, IDisposable
{
    static readonly TimeSpan PollInterval = TimeSpan.FromSeconds(2);

    readonly Func<MonitorsLayout> _newLayout;
    readonly ILayoutPersistence _persistence;
    readonly ILinuxMonitorSource? _source;
    readonly Timer _pollTimer;
    string _lastPlugSignature;

    public LinuxLayoutFactory(Func<MonitorsLayout> newLayout, ILayoutPersistence persistence)
    {
        _newLayout = newLayout;
        _persistence = persistence;

        _source = new ILinuxMonitorSource[] { new KScreenMonitorSource(), new XRandRMonitorSource() }
            .FirstOrDefault(s => s.IsAvailable());

        _lastPlugSignature = DrmEdidReader.PlugSignature();
        _pollTimer = new Timer(_ => PollPlugSignature(), null, PollInterval, PollInterval);
    }

    public event EventHandler? DisplayChanged;

    public event EventHandler? WallpaperChanged;

    /// <summary>
    /// Re-read the plasma wallpapers into an already-built layout, in place. Screens
    /// are matched by logical geometry: plasmashell's screenGeometry and our InPixel
    /// bounds live in the same kscreen coordinate space.
    /// </summary>
    public void UpdateWallpaper(MonitorsLayout layout)
    {
        var entries = PlasmaWallpaper.Query();
        if (entries.Count == 0) return;

        foreach (var source in layout.PhysicalSources)
        {
            var bounds = source.Source.InPixel.Bounds;
            var entry = entries.FirstOrDefault(e =>
                Math.Abs(e.X - bounds.X) < 2 && Math.Abs(e.Y - bounds.Y) < 2);
            if (entry == null) continue;

            source.Source.WallpaperPath = entry.ImagePath;
            // org.kde.image FillMode is a QML Image.fillMode value.
            source.Source.WallpaperStyle = entry.FillMode switch
            {
                0 => WallpaperStyle.Stretch,
                1 => WallpaperStyle.Fit,
                3 or 4 or 5 => WallpaperStyle.Tile,
                6 => WallpaperStyle.Center,
                _ => WallpaperStyle.Fill,
            };
        }
    }

    public void Dispose() => _pollTimer.Dispose();

    // Moving, scaling or toggling an output in the system settings never touches the
    // sysfs connectors, so the plug poll can't see it. Both major desktops persist the
    // applied output layout to a config file the moment it changes: KWin (Plasma 6) to
    // kwinoutputconfig.json, GNOME/mutter to monitors.xml. A cheap mtime stat per tick
    // is our display-change notification; spurious rewrites (the KWin file also stores
    // brightness…) settle to an identical DisplaySignature and are absorbed by
    // MainService's idempotence guard, like spurious WM_DISPLAYCHANGE on Windows.
    static readonly string[] OutputConfigPaths =
    [
        Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "kwinoutputconfig.json"),
        Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "monitors.xml"),
    ];

    void PollPlugSignature()
    {
        var signature = DrmEdidReader.PlugSignature();
        if (signature != _lastPlugSignature)
        {
            _lastPlugSignature = signature;
            DisplayChanged?.Invoke(this, EventArgs.Empty);
        }

        // GetLastWriteTimeUtc doesn't throw on missing files (it returns a constant
        // sentinel), so absent config files simply never fire.
        var outputStamp = 0L;
        foreach (var path in OutputConfigPaths)
        {
            var ticks = File.GetLastWriteTimeUtc(path).Ticks;
            if (ticks > outputStamp) outputStamp = ticks;
        }
        if (_lastOutputConfigStamp != 0 && outputStamp != _lastOutputConfigStamp)
            DisplayChanged?.Invoke(this, EventArgs.Empty);
        _lastOutputConfigStamp = outputStamp;

        // Plasma rewrites its containment config when the wallpaper changes: a cheap
        // mtime stat per tick is the Linux equivalent of the Windows registry watcher.
        try
        {
            var stamp = File.GetLastWriteTimeUtc(PlasmaWallpaper.ConfigPath).Ticks;
            if (_lastWallpaperStamp != 0 && stamp != _lastWallpaperStamp)
                WallpaperChanged?.Invoke(this, EventArgs.Empty);
            _lastWallpaperStamp = stamp;
        }
        catch
        {
            // no plasma config: nothing to watch
        }
    }

    long _lastWallpaperStamp;
    long _lastOutputConfigStamp;

    /// <summary>The active discovery backend name ("kscreen"/"xrandr"), for the Info view.</summary>
    public string? SourceName => _source?.Name;

    /// <summary>Fresh raw discovery data, for the Info view.</summary>
    public IReadOnlyList<LinuxMonitor> QueryMonitors() => _source?.Query() ?? [];

    /// <summary>
    /// Raise <see cref="DisplayChanged"/> for a change the sysfs plug poll cannot see
    /// (primary/priority, enable/disable, position — the connectors don't move).
    /// Called by <see cref="LinuxDisplayController"/> after a successful topology
    /// command; MainService's settle/idempotence logic absorbs the async apply.
    /// </summary>
    public void NotifyDisplayChanged() => DisplayChanged?.Invoke(this, EventArgs.Empty);

    public MonitorsLayout Create()
    {
        var layout = _newLayout();

        var monitors = _source?.Query() ?? [];
        if (monitors.Count == 0)
        {
            // No discovery available: still open the UI on a plausible single monitor
            // rather than an empty editor.
            monitors = [new LinuxMonitor
            {
                ConnectorName = "FALLBACK",
                LogicalWidth = 1920, LogicalHeight = 1080,
                PixelWidth = 1920, PixelHeight = 1080,
                WidthMm = 527, HeightMm = 296,
                Primary = true
            }];
        }

        foreach (var monitor in monitors)
            layout.AddMonitor(monitor);

        layout.Id = layout.ComputeId();

        // Same ordering as Windows: infer positions from the system topology, then let the
        // saved layout override them, then re-anchor on the current primary.
        layout.SetLocationsFromSystemConfiguration();
        _persistence.Load(layout);
        layout.AnchorOnPrimary();

        UpdateWallpaper(layout);

        return layout;
    }

    /// <inheritdoc/>
    public string DisplaySignature()
        => _source == null
            ? ""
            : string.Join("|", _source.Query()
                .OrderBy(m => m.ConnectorName, StringComparer.Ordinal)
                .Select(m => $"{m.ConnectorName}[{m.LogicalX},{m.LogicalY} {m.PixelWidth}x{m.PixelHeight}@{m.Scale}]"
                             + $"{(m.Primary ? "*" : "")}{(m.Enabled ? "" : "-")}d{m.WidthMm}x{m.HeightMm}"));
}

/// <summary>
/// <see cref="LinuxMonitor"/> → model mapping, the Linux counterpart of WindowsLayoutMapping.
/// Identity comes from the EDID (PnP code, serial) so persistence keys survive replugging a
/// monitor into another port; the connector name is the fallback for EDID-less outputs.
/// The pixel space fed to the model is the compositor's logical space — the space the
/// cursor actually moves in under Wayland, and plain pixels on native X11.
/// </summary>
public static class LinuxLayoutMapping
{
    public static void AddMonitor(this MonitorsLayout layout, LinuxMonitor monitor)
    {
        var edid = monitor.Edid;

        var pnpCode = !string.IsNullOrEmpty(edid?.ManufacturerCode)
            ? $"{edid.ManufacturerCode}{edid.ProductCode}"
            : monitor.ConnectorName;

        var monitorId = edid != null
            ? $"{pnpCode}_{(string.IsNullOrEmpty(edid.SerialNumber) ? edid.Serial : edid.SerialNumber)}"
            : monitor.ConnectorName;

        // Two identical monitors can report the same serial: disambiguate by connector.
        if (layout.PhysicalMonitors.Any(m => m.Id == monitorId))
            monitorId = $"{monitorId}@{monitor.ConnectorName}";

        var model = layout.GetOrAddPhysicalMonitorModel(pnpCode, s =>
        {
            var m = new PhysicalMonitorModel(s)
            {
                PnpDeviceName = !string.IsNullOrEmpty(edid?.Model) ? edid.Model : monitor.ConnectorName
            };

            var widthMm = edid is { PhysicalWidth: > 0 } ? edid.PhysicalWidth : monitor.WidthMm;
            var heightMm = edid is { PhysicalHeight: > 0 } ? edid.PhysicalHeight : monitor.HeightMm;
            if (monitor.Orientation % 2 != 0 && edid != null) (widthMm, heightMm) = (heightMm, widthMm);

            if (widthMm > 0 && heightMm > 0)
            {
                var fixedRatio = m.PhysicalSize.FixedAspectRatio;
                m.PhysicalSize.FixedAspectRatio = false;
                m.PhysicalSize.Width = widthMm;
                m.PhysicalSize.Height = heightMm;
                m.PhysicalSize.FixedAspectRatio = fixedRatio;
            }

            if (!string.IsNullOrEmpty(edid?.ManufacturerCode))
                m.Logo = $"icon/Pnp/{edid.ManufacturerCode}?icon/Pnp/LBM";

            return m;
        });

        var physicalMonitor = new PhysicalMonitor(monitorId, layout, model)
        {
            DeviceId = monitor.ConnectorName,
            SerialNumber = edid?.SerialNumber is { Length: > 0 } sn ? sn : edid?.Serial ?? "N/A"
        };

        var source = new DisplaySource(monitorId)
        {
            InterfacePath = monitor.ConnectorName,
            DeviceName = monitor.ConnectorName,
            DisplayName = monitor.ConnectorName,
            SourceName = $"{edid?.VideoInterface ?? "Unknown"}:{monitor.ConnectorName}",
            SourceNumber = (layout.PhysicalMonitors.Count + 1).ToString(),
            Primary = monitor.Primary,
            AttachedToDesktop = monitor.Enabled,
            Orientation = monitor.Orientation,
            DisplayFrequency = monitor.Frequency
        };
        source.InPixel.Set(new HLab.Geo.Rect(
            new HLab.Geo.Point(monitor.LogicalX, monitor.LogicalY),
            new HLab.Geo.Size(monitor.LogicalWidth, monitor.LogicalHeight)));

        // Effective DPI mirrors the Windows semantic (the OS scaling): 96 per scale unit.
        // Raw DPI is the panel's real density, mode pixels against the EDID millimeters.
        var effectiveDpi = 96.0 * monitor.Scale;
        source.EffectiveDpi.Set(effectiveDpi, effectiveDpi);
        source.DpiAwareAngularDpi.Set(effectiveDpi, effectiveDpi);

        var widthForDpi = edid is { PhysicalWidth: > 0 } ? edid.PhysicalWidth : monitor.WidthMm;
        var heightForDpi = edid is { PhysicalHeight: > 0 } ? edid.PhysicalHeight : monitor.HeightMm;
        if (monitor.Orientation % 2 != 0 && edid != null) (widthForDpi, heightForDpi) = (heightForDpi, widthForDpi);
        source.RawDpi.Set(
            widthForDpi > 0 ? monitor.PixelWidth * 25.4 / widthForDpi : effectiveDpi,
            heightForDpi > 0 ? monitor.PixelHeight * 25.4 / heightForDpi : effectiveDpi);

        var physicalSource = new PhysicalSource(monitor.ConnectorName, physicalMonitor, source);
        physicalMonitor.ActiveSource = physicalSource;
        physicalMonitor.Sources.Add(physicalSource);

        layout.AddOrUpdatePhysicalMonitor(physicalMonitor);
        layout.AddOrUpdatePhysicalSource(physicalSource);
    }
}
