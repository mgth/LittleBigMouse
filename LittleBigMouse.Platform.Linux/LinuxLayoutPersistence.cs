#nullable enable
using LittleBigMouse.Plugins;
using LittleBigMouse.Plugins.Persistence;

namespace LittleBigMouse.Platform.Linux;

/// <summary>
/// Linux implementation of <see cref="ILayoutPersistence"/>: the shared
/// <see cref="LayoutPersistence"/> engine over <see cref="JsonLayoutStore"/> (JSON files
/// under ~/.config/LittleBigMouse). The excluded-process list stays a plain text file in
/// the XDG data dir — the daemon reads it. Autostart (the XDG entry) is the agent's.
/// </summary>
public class LinuxLayoutPersistence() : LayoutPersistence(new JsonLayoutStore());
