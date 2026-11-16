#nullable enable
using System.Security.Principal;
using LittleBigMouse.Plugins;
using LittleBigMouse.Plugins.Persistence;

#pragma warning disable CA1416

namespace LittleBigMouse.Platform.Windows;

/// <summary>
/// Windows implementation of <see cref="ILayoutPersistence"/>: the shared
/// <see cref="LayoutPersistence"/> engine over <see cref="RegistryLayoutStore"/>
/// (HKCU\SOFTWARE\Mgth\LittleBigMouse, byte-for-byte compatible with the historical
/// registry layout). The scheduled task that starts with the session is the agent's
/// (v6): this process neither registers nor reads it.
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
}
