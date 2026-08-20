#nullable enable
using System.Collections.Generic;
using System.Linq;
using LittleBigMouse.Plugins.Persistence;
using Microsoft.Win32;

#pragma warning disable CA1416

namespace LittleBigMouse.Platform.Windows;

/// <summary>
/// Windows <see cref="ILayoutStore"/>: the historical HKCU\SOFTWARE\Mgth\LittleBigMouse
/// registry tree, byte-for-byte (invariant-culture strings, bools as "1"/"0"). Key and
/// value names are unchanged from the former <c>PersistencyExtensions</c>, so existing
/// user data is read as-is. Legacy locations are honored on read — options that once
/// lived in the layout key, and the old "ShowAttachDetachWarning" name — and migrate to
/// the current location at the next write. Unlike the historical GetOrSet code, reads
/// never seed the registry: values appear at the first save.
/// </summary>
public class RegistryLayoutStore : ILayoutStore
{
    const string RootKey = @"SOFTWARE\Mgth\LittleBigMouse";

    static RegistryKey? OpenRoot(bool create)
        => create ? Registry.CurrentUser.CreateSubKey(RootKey) : Registry.CurrentUser.OpenSubKey(RootKey);

    //==================//
    // Read             //
    //==================//

    public LayoutStoreData Read(string layoutId, IReadOnlyCollection<string> pnpCodes)
    {
        var data = new LayoutStoreData();

        using var root = OpenRoot(create: false);
        if (root == null) return data;

        using var layoutKey = root.OpenSubKey(@$"Layouts\{layoutId}");

        data.GlobalOptions = ReadGlobalOptions(root, layoutKey);
        if (layoutKey != null) data.Layout = ReadLayout(layoutKey);

        foreach (var pnpCode in pnpCodes)
        {
            using var modelKey = root.OpenSubKey(@$"monitors\{pnpCode}");
            if (modelKey != null) data.Models[pnpCode] = ReadModel(modelKey);
        }

        return data;
    }

    static GlobalOptionsDto ReadGlobalOptions(RegistryKey root, RegistryKey? layoutKey) => new()
    {
        DaemonPort = root.TryGetInt("DaemonPort"),
        // These options historically lived in the layout key: fall back to it so old
        // configs keep their values (they reach the root key at the next save).
        Priority = root.TryGetString("Priority") ?? layoutKey?.TryGetString("Priority"),
        PriorityUnhooked = root.TryGetString("PriorityUnhooked") ?? layoutKey?.TryGetString("PriorityUnhooked"),
        HomeCinema = root.TryGetBool("HomeCinema") ?? layoutKey?.TryGetBool("HomeCinema"),
        Pinned = root.TryGetBool("Pinned") ?? layoutKey?.TryGetBool("Pinned"),
        AutoUpdate = root.TryGetBool("AutoUpdate") ?? layoutKey?.TryGetBool("AutoUpdate"),
        StartMinimized = root.TryGetBool("StartMinimized") ?? layoutKey?.TryGetBool("StartMinimized"),
        StartElevated = root.TryGetBool("StartElevated") ?? layoutKey?.TryGetBool("StartElevated"),
        DebugTools = root.TryGetBool("DebugTools"),
        VcpControl = root.TryGetBool("VcpControl"),
        // "ShowAttachDetachWarning" is the former name of the option, read as fallback.
        ShowMonitorActionWarning = root.TryGetBool("ShowMonitorActionWarning") ?? root.TryGetBool("ShowAttachDetachWarning"),
        BorderValues = root.TryGetString("BorderValues"),
        HideTrayIcon = root.TryGetBool("HideTrayIcon"),
        ExcludedDefaultsVersion = root.TryGetInt("ExcludedDefaultsVersion")
    };

    static LayoutDto ReadLayout(RegistryKey key)
    {
        var dto = new LayoutDto
        {
            Options = new LayoutOptionsDto
            {
                AllowOverlaps = key.TryGetBool("AllowOverlaps"),
                AllowDiscontinuity = key.TryGetBool("AllowDiscontinuity"),
                Algorithm = key.TryGetString("Algorithm"),
                MaxTravelDistance = key.TryGet("MaxTravelDistance"),
                FreelookCheckInterval = key.TryGet("FreelookCheckInterval"),
                FreelookEnabled = key.TryGetBool("FreelookEnabled"),
                LoopX = key.TryGetBool("LoopX"),
                LoopY = key.TryGetBool("LoopY"),
                Enabled = key.TryGetBool("Enabled"),
                AdjustPointer = key.TryGetBool("AdjustPointer"),
                AdjustSpeed = key.TryGetBool("AdjustSpeed"),
                Priority = key.TryGetString("Priority"),
                PriorityUnhooked = key.TryGetString("PriorityUnhooked")
            }
        };

        using var monitors = key.OpenSubKey("PhysicalMonitors");
        if (monitors == null) return dto;

        foreach (var id in monitors.GetSubKeyNames())
        {
            using var monitorKey = monitors.OpenSubKey(id);
            if (monitorKey != null) dto.Monitors[id] = ReadMonitor(monitorKey);
        }

        return dto;
    }

    static MonitorDto ReadMonitor(RegistryKey key)
    {
        var dto = new MonitorDto
        {
            XLocationInMm = key.TryGet("XLocationInMm"),
            YLocationInMm = key.TryGet("YLocationInMm"),
            PhysicalRatioX = key.TryGet("PhysicalRatioX"),
            PhysicalRatioY = key.TryGet("PhysicalRatioY"),
            BorderResistance = ReadBorderResistance(key),
            // Presence of Borders\Left IS the "monitor owns its bezel borders" flag
            // (BordersCustomized) — a partial subkey without Left does not count.
            Borders = key.TryGet(@"Borders\Left") is { } left
                ? new BordersDto
                {
                    Left = left,
                    Top = key.TryGet(@"Borders\Top"),
                    Right = key.TryGet(@"Borders\Right"),
                    Bottom = key.TryGet(@"Borders\Bottom")
                }
                : null,
            ActiveSource = key.TryGetString("ActiveSource"),
            SerialNumber = key.TryGetString("SerialNumber"),
            ExcludedFromLayout = key.TryGetBool("ExcludedFromLayout"),
            Sources = []
        };

        // Sources are stored as sibling subkeys of the two border subkeys.
        foreach (var id in key.GetSubKeyNames())
        {
            if (id is "BorderResistance" or "Borders") continue;

            using var sourceKey = key.OpenSubKey(id);
            if (sourceKey == null) continue;

            dto.Sources[id] = new SourceDto
            {
                PixelX = sourceKey.TryGet("PixelX"),
                PixelY = sourceKey.TryGet("PixelY"),
                PixelWidth = sourceKey.TryGet("PixelWidth"),
                PixelHeight = sourceKey.TryGet("PixelHeight"),
                Orientation = sourceKey.TryGetInt("Orientation"),
                DisplayName = sourceKey.TryGetString("DisplayName"),
                Primary = sourceKey.TryGetBool("Primary")
            };
        }

        return dto;
    }

    static ModelDto ReadModel(RegistryKey key) => new()
    {
        Width = key.TryGet(@"Size\Width"),
        Height = key.TryGet(@"Size\Height"),
        Borders = ReadBorders(key, "Borders"),
        PnpName = key.TryGetString("PnpName")
    };

    static BordersDto? ReadBorders(RegistryKey key, string name)
    {
        using var sub = key.OpenSubKey(name);
        if (sub == null) return null;

        return new BordersDto
        {
            Left = sub.TryGet("Left"),
            Top = sub.TryGet("Top"),
            Right = sub.TryGet("Right"),
            Bottom = sub.TryGet("Bottom")
        };
    }

    static BorderResistanceDto? ReadBorderResistance(RegistryKey key)
    {
        using var sub = key.OpenSubKey("BorderResistance");
        if (sub == null) return null;

        return new BorderResistanceDto
        {
            Left = ReadSide(sub, "Left"),
            Top = ReadSide(sub, "Top"),
            Right = ReadSide(sub, "Right"),
            Bottom = ReadSide(sub, "Bottom")
        };
    }

    /// <summary>
    /// One edge. Before the move/drag split each edge was a single VALUE holding
    /// its resistance; it is now a SUBKEY. Reading the value first is what
    /// migrates an existing installation — one number meant "resist any crossing",
    /// so it maps to both modes. See BorderSideDtoJsonConverter for the Linux twin.
    /// </summary>
    static BorderSideDto? ReadSide(RegistryKey borderResistance, string name)
    {
        if (borderResistance.TryGet(name) is { } legacy)
            return new BorderSideDto { Move = legacy, Drag = legacy };

        using var sub = borderResistance.OpenSubKey(name);
        if (sub == null) return null;

        return new BorderSideDto
        {
            Move = sub.TryGet("Move"),
            MoveBlock = sub.TryGetBool("MoveBlock"),
            Drag = sub.TryGet("Drag"),
            DragBlock = sub.TryGetBool("DragBlock"),
            Sections = ReadSections(sub)
        };
    }

    static List<BorderSectionDto>? ReadSections(RegistryKey side)
    {
        using var container = side.OpenSubKey("Sections");
        if (container == null) return null;

        var sections = new List<BorderSectionDto>();

        // Subkeys are named by index; order matters for nothing but stability, and
        // GetSubKeyNames is alphabetical ("10" before "2"), hence the sort.
        foreach (var name in container.GetSubKeyNames()
                     .OrderBy(n => int.TryParse(n, out var i) ? i : int.MaxValue))
        {
            using var sectionKey = container.OpenSubKey(name);
            if (sectionKey == null) continue;

            sections.Add(new BorderSectionDto
            {
                From = sectionKey.TryGet("From"),
                To = sectionKey.TryGet("To"),
                Move = sectionKey.TryGet("Move"),
                MoveBlock = sectionKey.TryGetBool("MoveBlock"),
                Drag = sectionKey.TryGet("Drag"),
                DragBlock = sectionKey.TryGetBool("DragBlock")
            });
        }

        return sections;
    }

    //==================//
    // Write            //
    //==================//

    public void WriteGlobalOptions(GlobalOptionsDto o)
    {
        using var root = OpenRoot(create: true);
        if (root == null) return;

        Set(root, "DaemonPort", o.DaemonPort);
        Set(root, "Priority", o.Priority);
        Set(root, "PriorityUnhooked", o.PriorityUnhooked);
        Set(root, "HomeCinema", o.HomeCinema);
        Set(root, "Pinned", o.Pinned);
        Set(root, "AutoUpdate", o.AutoUpdate);
        Set(root, "StartMinimized", o.StartMinimized);
        Set(root, "StartElevated", o.StartElevated);
        Set(root, "DebugTools", o.DebugTools);
        Set(root, "VcpControl", o.VcpControl);
        Set(root, "ShowMonitorActionWarning", o.ShowMonitorActionWarning);
        Set(root, "BorderValues", o.BorderValues);
        Set(root, "HideTrayIcon", o.HideTrayIcon);
        Set(root, "ExcludedDefaultsVersion", o.ExcludedDefaultsVersion);
    }

    public void WriteLayout(string layoutId, LayoutDto layout)
    {
        using var root = OpenRoot(create: true);
        using var key = root?.CreateSubKey(@$"Layouts\{layoutId}");
        if (key == null) return;

        if (layout.Options is { } o)
        {
            Set(key, "AllowOverlaps", o.AllowOverlaps);
            Set(key, "AllowDiscontinuity", o.AllowDiscontinuity);
            Set(key, "Algorithm", o.Algorithm);
            Set(key, "MaxTravelDistance", o.MaxTravelDistance);
            Set(key, "FreelookCheckInterval", o.FreelookCheckInterval);
            Set(key, "FreelookEnabled", o.FreelookEnabled);
            Set(key, "LoopX", o.LoopX);
            Set(key, "LoopY", o.LoopY);
            Set(key, "Enabled", o.Enabled);
            Set(key, "AdjustPointer", o.AdjustPointer);
            Set(key, "AdjustSpeed", o.AdjustSpeed);

            // Priority/PriorityUnhooked belong to the root key. Deleting is what ends the
            // migration ReadGlobalOptions has been half-doing for versions: Set() skips a
            // null but never removes, so a value left here would go on overriding the root
            // one at every load. Same reason WriteSide deletes the pre-split edge value.
            key.DeleteValue("Priority", throwOnMissingValue: false);
            key.DeleteValue("PriorityUnhooked", throwOnMissingValue: false);
        }

        foreach (var (id, monitor) in layout.Monitors)
        {
            using var monitorKey = key.CreateSubKey(@$"PhysicalMonitors\{id}");
            WriteMonitor(monitorKey, monitor);
        }
    }

    static void WriteMonitor(RegistryKey key, MonitorDto m)
    {
        Set(key, "XLocationInMm", m.XLocationInMm);
        Set(key, "YLocationInMm", m.YLocationInMm);
        Set(key, "PhysicalRatioX", m.PhysicalRatioX);
        Set(key, "PhysicalRatioY", m.PhysicalRatioY);
        WriteBorderResistance(key, m.BorderResistance);
        // Presence is the BordersCustomized flag: nothing is written for null, so an
        // uncustomized monitor keeps mirroring its model on the next load.
        WriteBorders(key, "Borders", m.Borders);
        Set(key, "ActiveSource", m.ActiveSource);
        Set(key, "SerialNumber", m.SerialNumber);
        Set(key, "ExcludedFromLayout", m.ExcludedFromLayout);

        if (m.Sources == null) return;
        foreach (var (id, s) in m.Sources)
        {
            Set(key, @$"{id}\PixelX", s.PixelX);
            Set(key, @$"{id}\PixelY", s.PixelY);
            Set(key, @$"{id}\PixelWidth", s.PixelWidth);
            Set(key, @$"{id}\PixelHeight", s.PixelHeight);
            Set(key, @$"{id}\Orientation", s.Orientation);
            Set(key, @$"{id}\DisplayName", s.DisplayName);
            Set(key, @$"{id}\Primary", s.Primary);
        }
    }

    public void WriteModels(IReadOnlyDictionary<string, ModelDto> models)
    {
        using var root = OpenRoot(create: true);
        if (root == null) return;

        foreach (var (pnpCode, model) in models)
        {
            using var key = root.CreateSubKey(@$"monitors\{pnpCode}");
            WriteBorders(key, "Borders", model.Borders);
            Set(key, @"Size\Width", model.Width);
            Set(key, @"Size\Height", model.Height);
            Set(key, "PnpName", model.PnpName);
        }
    }

    static void WriteBorders(RegistryKey key, string name, BordersDto? borders)
    {
        if (borders == null) return;
        Set(key, @$"{name}\Left", borders.Left);
        Set(key, @$"{name}\Top", borders.Top);
        Set(key, @$"{name}\Right", borders.Right);
        Set(key, @$"{name}\Bottom", borders.Bottom);
    }

    static void WriteBorderResistance(RegistryKey key, BorderResistanceDto? resistance)
    {
        if (resistance == null) return;

        using var sub = key.CreateSubKey("BorderResistance");
        if (sub == null) return;

        WriteSide(sub, "Left", resistance.Left);
        WriteSide(sub, "Top", resistance.Top);
        WriteSide(sub, "Right", resistance.Right);
        WriteSide(sub, "Bottom", resistance.Bottom);
    }

    static void WriteSide(RegistryKey borderResistance, string name, BorderSideDto? side)
    {
        if (side == null) return;

        // The pre-split VALUE of the same name must go, or ReadSide would keep
        // preferring it and the edge would be frozen on its migrated setting. A
        // registry key can hold a value and a subkey with the same name at once,
        // so this is not hypothetical.
        borderResistance.DeleteValue(name, throwOnMissingValue: false);

        using var sideKey = borderResistance.CreateSubKey(name);
        if (sideKey == null) return;

        Set(sideKey, "Move", side.Move);
        Set(sideKey, "MoveBlock", side.MoveBlock);
        Set(sideKey, "Drag", side.Drag);
        Set(sideKey, "DragBlock", side.DragBlock);

        // Rewritten wholesale: leftover subkeys from a longer previous list would
        // come back as phantom sections on the next load.
        sideKey.DeleteSubKeyTree("Sections", throwOnMissingSubKey: false);

        if (side.Sections is not { Count: > 0 } sections) return;

        for (var i = 0; i < sections.Count; i++)
        {
            using var sectionKey = sideKey.CreateSubKey(@$"Sections\{i}");
            if (sectionKey == null) continue;

            Set(sectionKey, "From", sections[i].From);
            Set(sectionKey, "To", sections[i].To);
            Set(sectionKey, "Move", sections[i].Move);
            Set(sectionKey, "MoveBlock", sections[i].MoveBlock);
            Set(sectionKey, "Drag", sections[i].Drag);
            Set(sectionKey, "DragBlock", sections[i].DragBlock);
        }
    }

    static void Set(RegistryKey key, string name, string? value)
    {
        if (value != null) key.SetKey(name, value);
    }

    static void Set(RegistryKey key, string name, double? value)
    {
        if (value is { } v) key.SetKey(name, v);
    }

    static void Set(RegistryKey key, string name, bool? value)
    {
        if (value is { } v) key.SetKey(name, v);
    }

    static void Set(RegistryKey key, string name, int? value)
    {
        if (value is { } v) key.SetKey(name, v);
    }
}
