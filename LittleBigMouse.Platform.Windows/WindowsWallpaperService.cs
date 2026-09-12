#nullable enable
using System;
using LittleBigMouse.Plugins;

namespace LittleBigMouse.Platform.Windows;

/// <summary>
/// Windows sets the wallpaper per screen through the shell's IDesktopWallpaper, so the
/// editor is always worth offering here.
/// <para>
/// Setting it is the agent's (v6). The Windows half of that is not ported yet: the agent
/// answers "unsupported" and leaves the desktop alone, so nothing here reaches for
/// IDesktopWallpaper any more — it is the agent that will.
/// </para>
/// </summary>
public class WindowsWallpaperService : IWallpaperService
{
    public bool IsSupported => OperatingSystem.IsWindows();
}
