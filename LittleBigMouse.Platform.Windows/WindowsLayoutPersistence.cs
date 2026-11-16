#nullable enable
using System.Security.Principal;
using LittleBigMouse.Plugins;
using LittleBigMouse.Plugins.Persistence;

#pragma warning disable CA1416

namespace LittleBigMouse.Platform.Windows;

/// <summary>
/// Windows implementation of <see cref="ILayoutPersistence"/>: the shared
/// <see cref="LayoutPersistence"/> engine over the agent's JSON store
/// (%LOCALAPPDATA%\Mgth\LittleBigMouse, decision D2). The window reads what the agent
/// writes: reading the 5.x registry instead showed a layout the agent no longer used, and
/// lost every save the agent made. Until the agent's first launch imports the registry,
/// the registry is read (<see cref="LegacyFallbackLayoutStore"/>). The scheduled task that
/// starts with the session is the agent's too: this process neither registers nor reads it.
/// </summary>
public class WindowsLayoutPersistence() : LayoutPersistence(
    new LegacyFallbackLayoutStore(new JsonLayoutStore(), () => new RegistryLayoutStore()))
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
