#nullable enable
using LittleBigMouse.Plugins;
using LittleBigMouse.Plugins.Persistence;

namespace LittleBigMouse.Platform.Linux;

/// <summary>
/// Linux implementation of <see cref="ILayoutPersistence"/>: the shared
/// <see cref="LayoutPersistence"/> engine over <see cref="JsonLayoutStore"/> (JSON files
/// under ~/.config/LittleBigMouse). The excluded-process list stays a plain text file in
/// the XDG data dir — the daemon reads it. Autostart (systemd user unit / .desktop entry)
/// is not implemented on Linux yet: the base no-op hooks apply.
/// </summary>
public class LinuxLayoutPersistence() : LayoutPersistence(new JsonLayoutStore());
