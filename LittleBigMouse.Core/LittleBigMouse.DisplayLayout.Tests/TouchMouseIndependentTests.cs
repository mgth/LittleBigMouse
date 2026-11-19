using System.Xml.Linq;
using DynamicData;
using LittleBigMouse.DisplayLayout.Dimensions;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.DisplayLayout.Monitors.Extensions;
using LittleBigMouse.Plugins.Persistence;
using Xunit;

namespace LittleBigMouse.DisplayLayout.Tests;

public class TouchMouseIndependentTests
{
    [Fact]
    public void OldSettingsLeaveTouchSeparationDisabled()
    {
        var options = new ILayoutOptions.Design();
        LayoutDtoMapper.Apply(options, new GlobalOptionsDto());
        Assert.False(options.TouchMouseIndependent);
        Assert.False(options.StylusMouseIndependent);

        Assert.False(options.RestoreKeyboardFocus);
        Assert.Equal(120, options.FocusRestoreDelay);
        Assert.False(options.FocusRestoreOnMouseMove);
        Assert.True(options.TouchAllDisplays);
        Assert.Equal("None", options.TouchOverrideModifier);
    }

    [Theory]
    [InlineData(true)]
    [InlineData(false)]
    public void SettingSurvivesPersistenceAndReachesWire(bool enabled)
    {
        var options = new ILayoutOptions.Design { TouchMouseIndependent = enabled, StylusMouseIndependent = enabled, RestoreKeyboardFocus = enabled };
        var saved = LayoutDtoMapper.ToGlobalOptionsDto(options, null);
        var restored = new ILayoutOptions.Design { TouchMouseIndependent = !enabled, RestoreKeyboardFocus = !enabled };
        LayoutDtoMapper.Apply(restored, saved);
        Assert.Equal(enabled, restored.TouchMouseIndependent);
        Assert.Equal(enabled, restored.StylusMouseIndependent);
        Assert.Equal(enabled, restored.RestoreKeyboardFocus);
        using var layout = new MonitorsLayout(restored);
        var zones = layout.ComputeZones();
        Assert.Equal(enabled.ToString(),
            XDocument.Parse(zones.Serialize()).Root!.Attribute("StylusMouseIndependent")!.Value);
        Assert.Equal(enabled.ToString(),
            XDocument.Parse(zones.Serialize()).Root!.Attribute("TouchMouseIndependent")!.Value);
        Assert.Equal(enabled.ToString(),
            XDocument.Parse(zones.Serialize()).Root!.Attribute("RestoreKeyboardFocus")!.Value);
    }

    [Fact]
    public void TouchPreferencesRoundTripAndDoNotChangeGeneralExclusions()
    {
        var options = new ILayoutOptions.Design
        {
            FocusRestoreDelay = 275, FocusRestoreOnMouseMove = true,
            TouchAllDisplays = false, TouchDisplayIds = "SRC1",
            TouchOverrideModifier = "Shift", FocusKeepApps = "chrome.exe\nnotepad.exe",
            FocusRestoreApps = "control.exe"
        };
        var restored = new ILayoutOptions.Design();
        LayoutDtoMapper.Apply(restored, LayoutDtoMapper.ToGlobalOptionsDto(options, null));
        Assert.Equal(275, restored.FocusRestoreDelay);
        Assert.True(restored.FocusRestoreOnMouseMove);
        Assert.False(restored.TouchAllDisplays);
        Assert.Equal("SRC1", restored.TouchDisplayIds);
        Assert.Equal("Shift", restored.TouchOverrideModifier);
        Assert.Equal(options.FocusKeepApps, restored.FocusKeepApps);
        Assert.Equal(options.FocusRestoreApps, restored.FocusRestoreApps);
        Assert.Equal(new ILayoutOptions.Design().ExcludedList, restored.ExcludedList);
        using var layout = new MonitorsLayout(restored);
        var root = XDocument.Parse(layout.ComputeZones().Serialize()).Root!;
        Assert.Equal("275", root.Attribute("FocusRestoreDelay")!.Value);
        Assert.Equal("True", root.Attribute("FocusRestoreOnMouseMove")!.Value);
        Assert.Equal("False", root.Attribute("TouchAllDisplays")!.Value);
        Assert.Equal("SRC1", root.Attribute("TouchDisplayIds")!.Value);
        Assert.Equal("Shift", root.Attribute("TouchOverrideModifier")!.Value);
        Assert.Equal("chrome.exe;notepad.exe", root.Attribute("FocusKeepApps")!.Value);
        Assert.Equal("control.exe", root.Attribute("FocusRestoreApps")!.Value);
        Assert.Equal("", root.Attribute("TouchDisplayBounds")!.Value);
    }

    [Fact]
    public void SelectedSensorPanelIsIncludedEvenWhenExcludedFromCursorLayout()
    {
        var options = new ILayoutOptions.Design { TouchAllDisplays = false, TouchDisplayIds = "SRC1" };
        using var layout = new MonitorsLayout(options);
        var model = new PhysicalMonitorModel("TST1234");
        model.PhysicalSize.Width = 600; model.PhysicalSize.Height = 340;
        var monitor = new PhysicalMonitor("MON1", layout, model) { ExcludedFromLayout = true };
        var display = new DisplaySource("SRC1") { AttachedToDesktop = true };
        display.InPixel.Set(new HLab.Geo.Rect(-1920, 0, 1920, 1080));
        var source = new PhysicalSource("DEV1", monitor, display);
        monitor.ActiveSource = source;
        monitor.Sources.Add(source);
        layout.AddOrUpdatePhysicalMonitor(monitor);
        layout.AddOrUpdatePhysicalSource(source);
        var zones = layout.ComputeZones();
        Assert.Empty(zones.Zones);
        Assert.Equal("-1920,0,1920,1080", zones.TouchDisplayBounds);
        options.TouchDisplayIds = "unplugged";
        Assert.Equal("", layout.ComputeZones().TouchDisplayBounds);
    }
}
