#nullable enable
using System.Security.Principal;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Plugins;
using LittleBigMouse.Plugins.Persistence;

#pragma warning disable CA1416

namespace LittleBigMouse.Platform.Windows;

/// <summary>
/// Windows implementation of <see cref="ILayoutPersistence"/>: the shared
/// <see cref="LayoutPersistence"/> engine over <see cref="RegistryLayoutStore"/>
/// (HKCU\SOFTWARE\Mgth\LittleBigMouse, byte-for-byte compatible with the historical
/// registry layout), with autostart wired to the Task Scheduler
/// (<see cref="AutostartExtensions"/>).
/// </summary>
public class WindowsLayoutPersistence() : LayoutPersistence(new RegistryLayoutStore())
{
    protected override bool IsElevated
    {
        get
        {
            using var identity = WindowsIdentity.GetCurrent();
            return new WindowsPrincipal(identity).IsInRole(WindowsBuiltInRole.Administrator);
        }
    }

    protected override bool IsAutostartScheduled(IMonitorsLayout layout) => layout.IsScheduled();

    protected override void SetAutostart(IMonitorsLayout layout, bool enabled, bool elevated)
    {
        if (enabled) layout.Schedule(elevated);
        else layout.Unschedule();
    }
}
