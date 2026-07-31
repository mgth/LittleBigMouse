#nullable enable
using System;
using System.Text.Json;
using System.Text.Json.Serialization;
using DynamicData;
using HLab.Geo;
using LittleBigMouse.DisplayLayout.Dimensions;
using LittleBigMouse.DisplayLayout.Monitors;

namespace LittleBigMouse.Ui.Avalonia.Main;

/// <summary>
/// Rebuilds a MonitorsLayout from a configuration export, for on-screen inspection of a
/// layout coming from another machine (no hardware needed). Only the "Layout" part of the
/// export is used; devices and zones are ignored. The resulting layout is flagged with a
/// virtual <see cref="LayoutSource"/>: the persistence layer refuses to store it and the
/// daemon refuses to hook it — the guards live there, not here.
/// </summary>
public static class VirtualLayoutFactory
{
    static readonly JsonSerializerOptions JsonOptions = new()
    {
        NumberHandling = JsonNumberHandling.AllowNamedFloatingPointLiterals
    };

    public static MonitorsLayout FromJson(string json, LayoutSource layoutSource, string? origin = null)
    {
        if (layoutSource == LayoutSource.System)
            throw new ArgumentException("A parsed export is virtual by definition.", nameof(layoutSource));

        using var doc = JsonDocument.Parse(json);
        var root = doc.RootElement;

        // accept either a full export ({"Layout":...}) or a bare layout object
        var layoutEl = root.ValueKind == JsonValueKind.Object && root.TryGetProperty("Layout", out var l)
            ? l : root;

        var imported = layoutEl.TryGetProperty("Options", out var optEl)
            ? optEl.Deserialize<ILayoutOptions.Design>(JsonOptions) ?? new ILayoutOptions.Design()
            : new ILayoutOptions.Design();

        // A REACTIVE options instance, not the inert Design POCO: the layout's Saved
        // propagation and the options panel rely on change notifications — a dead
        // options object silently breaks the whole dirty-tracking chain (the save
        // button never lit up while a virtual layout was displayed).
        var options = ToReactiveOptions(imported);

        var layout = new MonitorsLayout(options)
        {
            Id = GetString(layoutEl, "Id", "virtual"),
            Source = layoutSource,
            SourceOrigin = origin
        };

        if (layoutEl.TryGetProperty("PhysicalMonitors", out var monitors)
            && monitors.ValueKind == JsonValueKind.Array)
        {
            foreach (var monitorEl in monitors.EnumerateArray())
            {
                var monitor = CreateMonitor(monitorEl, layout);
                layout.AddOrUpdatePhysicalMonitor(monitor);

                foreach (var source in monitor.Sources.Items)
                {
                    layout.AddOrUpdatePhysicalSource(source);
                }
            }
        }

        layout.UpdatePhysicalMonitors();

        // born saved: the save button must not light up for a layout that
        // should never be persisted (saving it would also re-run the startup
        // scheduling logic with the imported options)
        foreach (var monitor in layout.PhysicalMonitors)
        {
            foreach (var source in monitor.Sources.Items)
            {
                source.Source.Saved = true;
                source.Saved = true;
            }
            monitor.Saved = true;
        }
        options.Saved = true;
        layout.Saved = true;

        return layout;
    }

    /// <summary>
    /// Copy the LAYOUT-scoped options of the export (the set LayoutOptionsDto persists)
    /// onto a live LbmOptions. App/machine-scoped options (autostart, tray, VCP, update
    /// checks…) deliberately keep their defaults: they describe THIS machine's app, not
    /// the client's layout.
    /// </summary>
    static LbmOptions ToReactiveOptions(ILayoutOptions imported)
    {
        var options = new LbmOptions
        {
            AllowOverlaps = imported.AllowOverlaps,
            AllowDiscontinuity = imported.AllowDiscontinuity,
            Algorithm = imported.Algorithm,
            MaxTravelDistance = imported.MaxTravelDistance,
            MinimalMaxTravelDistance = imported.MinimalMaxTravelDistance,
            FreelookCheckInterval = imported.FreelookCheckInterval,
            FreelookEnabled = imported.FreelookEnabled,
            LoopX = imported.LoopX,
            LoopY = imported.LoopY,
            AdjustPointer = imported.AdjustPointer,
            AdjustSpeed = imported.AdjustSpeed,
            Priority = imported.Priority,
            PriorityUnhooked = imported.PriorityUnhooked,
            IsUnaryRatio = imported.IsUnaryRatio,

            // a virtual layout must never engage the engine on its own
            Enabled = false,

            // keep the debug tools visible while the virtual layout is displayed:
            // they are the only way to load another export or come back from one
            DebugTools = true
        };
        return options;
    }

    static PhysicalMonitor CreateMonitor(JsonElement monitorEl, MonitorsLayout layout)
    {
        var id = GetString(monitorEl, "Id", "monitor");

        var modelEl = monitorEl.TryGetProperty("Model", out var m) ? m : default;
        var pnpCode = GetString(modelEl, "PnpCode", id);

        var model = layout.GetOrAddPhysicalMonitorModel(pnpCode, s => new PhysicalMonitorModel(s));

        if (modelEl.ValueKind == JsonValueKind.Object)
        {
            if (modelEl.TryGetProperty("PhysicalSize", out var sizeEl))
            {
                var fixedRatio = model.PhysicalSize.FixedAspectRatio;
                model.PhysicalSize.FixedAspectRatio = false;

                model.PhysicalSize.Width = GetDouble(sizeEl, "Width", model.PhysicalSize.Width);
                model.PhysicalSize.Height = GetDouble(sizeEl, "Height", model.PhysicalSize.Height);
                model.PhysicalSize.TopBorder = GetDouble(sizeEl, "TopBorder", model.PhysicalSize.TopBorder);
                model.PhysicalSize.RightBorder = GetDouble(sizeEl, "RightBorder", model.PhysicalSize.RightBorder);
                model.PhysicalSize.BottomBorder = GetDouble(sizeEl, "BottomBorder", model.PhysicalSize.BottomBorder);
                model.PhysicalSize.LeftBorder = GetDouble(sizeEl, "LeftBorder", model.PhysicalSize.LeftBorder);

                model.PhysicalSize.FixedAspectRatio = fixedRatio;
            }

            model.PnpDeviceName = GetString(modelEl, "PnpDeviceName", pnpCode);
            model.Logo = GetString(modelEl, "Logo", "icon/Pnp/LBM");
        }

        var monitor = new PhysicalMonitor(id, layout, model)
        {
            DeviceId = GetString(monitorEl, "DeviceId", id),
            SerialNumber = GetString(monitorEl, "SerialNumber", "N/A")
        };

        var activeSourceId = monitorEl.TryGetProperty("ActiveSource", out var activeEl)
                             && activeEl.TryGetProperty("Source", out var activeSrcEl)
            ? GetString(activeSrcEl, "Id", id)
            : id;

        if (monitorEl.TryGetProperty("Sources", out var sourcesEl)
            && sourcesEl.TryGetProperty("Items", out var itemsEl)
            && itemsEl.ValueKind == JsonValueKind.Array)
        {
            foreach (var sourceEl in itemsEl.EnumerateArray())
            {
                var source = CreatePhysicalSource(sourceEl, monitor);
                if (source == null) continue;

                monitor.Sources.Add(source);
                if (monitor.ActiveSource == null || source.Source.Id == activeSourceId)
                    monitor.ActiveSource = source;
            }
        }

        // some exports may only carry ActiveSource
        if (monitor.ActiveSource == null && activeEl.ValueKind == JsonValueKind.Object)
        {
            var source = CreatePhysicalSource(activeEl, monitor);
            if (source != null)
            {
                monitor.Sources.Add(source);
                monitor.ActiveSource = source;
            }
        }

        // location must be set after the sources: DepthProjection is rebuilt
        // when ActiveSource changes, which would discard an earlier X/Y
        if (monitorEl.TryGetProperty("DepthRatio", out var ratioEl))
        {
            monitor.DepthRatio.X = GetDouble(ratioEl, "X", 1);
            monitor.DepthRatio.Y = GetDouble(ratioEl, "Y", 1);
        }

        if (monitorEl.TryGetProperty("DepthProjection", out var projectionEl))
        {
            monitor.DepthProjection.X = GetDouble(projectionEl, "X", 0);
            monitor.DepthProjection.Y = GetDouble(projectionEl, "Y", 0);
        }

        if (monitorEl.TryGetProperty("BorderResistance", out var resistanceEl))
        {
            var acrossMm = monitor.DepthProjection.Width;
            var downMm = monitor.DepthProjection.Height;

            ApplySide(monitor.BorderResistance.Left, resistanceEl, "Left", downMm);
            ApplySide(monitor.BorderResistance.Top, resistanceEl, "Top", acrossMm);
            ApplySide(monitor.BorderResistance.Right, resistanceEl, "Right", downMm);
            ApplySide(monitor.BorderResistance.Bottom, resistanceEl, "Bottom", acrossMm);
        }

        monitor.Placed = true;

        return monitor;
    }

    static PhysicalSource? CreatePhysicalSource(JsonElement physicalSourceEl, PhysicalMonitor monitor)
    {
        if (!physicalSourceEl.TryGetProperty("Source", out var sourceEl)) return null;

        var source = new DisplaySource(GetString(sourceEl, "Id", monitor.Id))
        {
            Primary = GetBool(sourceEl, "Primary"),
            AttachedToDesktop = GetBool(sourceEl, "AttachedToDesktop"),
            Orientation = (int)GetDouble(sourceEl, "Orientation", 0),
            DisplayFrequency = (int)GetDouble(sourceEl, "DisplayFrequency", 0),
            DeviceName = GetString(sourceEl, "DeviceName", ""),
            DisplayName = GetString(sourceEl, "DisplayName", ""),
            SourceName = GetString(sourceEl, "SourceName", ""),
            SourceNumber = GetString(sourceEl, "SourceNumber", ""),
            InterfaceName = GetString(sourceEl, "InterfaceName", ""),
            InterfaceLogo = GetString(sourceEl, "InterfaceLogo", ""),
            // the wallpaper file belongs to the exporting machine
            WallpaperPath = ""
        };

        if (sourceEl.TryGetProperty("InPixel", out var inPixelEl))
        {
            source.InPixel.Set(new Rect(
                new Point(GetDouble(inPixelEl, "X", 0), GetDouble(inPixelEl, "Y", 0)),
                new Size(GetDouble(inPixelEl, "Width", 0), GetDouble(inPixelEl, "Height", 0))));
        }

        SetRatio(source.EffectiveDpi, sourceEl, "EffectiveDpi");
        SetRatio(source.RawDpi, sourceEl, "RawDpi");
        SetRatio(source.DpiAwareAngularDpi, sourceEl, "DpiAwareAngularDpi");

        return new PhysicalSource(GetString(physicalSourceEl, "DeviceId", source.Id), monitor, source);
    }

    static void SetRatio(DisplayRatioValue ratio, JsonElement parent, string name)
    {
        if (!parent.TryGetProperty(name, out var el)) return;
        ratio.Set(GetDouble(el, "X", ratio.X), GetDouble(el, "Y", ratio.Y));
    }

    static string GetString(JsonElement el, string name, string fallback)
    {
        if (el.ValueKind != JsonValueKind.Object || !el.TryGetProperty(name, out var p)) return fallback;
        return p.ValueKind switch
        {
            JsonValueKind.String => p.GetString() ?? fallback,
            JsonValueKind.Null or JsonValueKind.Undefined => fallback,
            _ => p.GetRawText()
        };
    }

    /// <summary>
    /// One border-resistance edge from an exported layout. Diagnostics exports are
    /// kept around for a long time, so all three shapes this has taken are read: a
    /// bare number per edge, an object with a per-edge resistance, and the current
    /// list of sections. The first two describe a resistance over the whole edge,
    /// which is now expressed as a section spanning it.
    /// </summary>
    static void ApplySide(BorderSide side, JsonElement resistanceEl, string name, double edgeLengthMm)
    {
        if (resistanceEl.ValueKind != JsonValueKind.Object) return;
        if (!resistanceEl.TryGetProperty(name, out var sideEl)) return;

        if (sideEl.ValueKind == JsonValueKind.Number)
        {
            AddWholeEdge(sideEl.GetDouble(), false, sideEl.GetDouble(), false);
            return;
        }

        if (sideEl.ValueKind != JsonValueKind.Object) return;

        if (sideEl.TryGetProperty("Sections", out var sectionsEl)
            && sectionsEl.ValueKind == JsonValueKind.Array)
        {
            side.Sections.Edit(list =>
            {
                list.Clear();
                foreach (var sectionEl in sectionsEl.EnumerateArray())
                {
                    if (sectionEl.ValueKind != JsonValueKind.Object) continue;

                    list.Add(new BorderSection
                    {
                        From = GetDouble(sectionEl, "From", 0),
                        To = GetDouble(sectionEl, "To", 0),
                        Move = GetDouble(sectionEl, "Move", 0),
                        MoveBlock = GetBool(sectionEl, "MoveBlock"),
                        Drag = GetDouble(sectionEl, "Drag", 0),
                        DragBlock = GetBool(sectionEl, "DragBlock")
                    });
                }
            });

            if (side.Sections.Count > 0) return;
        }

        AddWholeEdge(
            GetDouble(sideEl, "Move", 0), GetBool(sideEl, "MoveBlock"),
            GetDouble(sideEl, "Drag", 0), GetBool(sideEl, "DragBlock"));

        return;

        void AddWholeEdge(double move, bool moveBlock, double drag, bool dragBlock)
        {
            if (edgeLengthMm <= 0) return;
            if (move <= 0 && drag <= 0 && !moveBlock && !dragBlock) return;

            side.Sections.Add(new BorderSection
            {
                From = 0,
                To = edgeLengthMm,
                Move = move,
                MoveBlock = moveBlock,
                Drag = drag,
                DragBlock = dragBlock
            });
        }
    }

    static double GetDouble(JsonElement el, string name, double fallback)
        => el.ValueKind == JsonValueKind.Object
           && el.TryGetProperty(name, out var p)
           && p.ValueKind == JsonValueKind.Number
            ? p.GetDouble()
            : fallback;

    static bool GetBool(JsonElement el, string name)
        => el.ValueKind == JsonValueKind.Object
           && el.TryGetProperty(name, out var p)
           && p.ValueKind == JsonValueKind.True;
}
