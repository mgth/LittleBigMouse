#nullable enable
using System;
using System.IO;

namespace LittleBigMouse.Plugins;

/// <summary>
/// The application's per-user directories, one convention per OS. Windows keeps the
/// historical %LOCALAPPDATA%\Mgth\LittleBigMouse for everything (settings live in the
/// registry there). Linux splits per the XDG spec: runtime data (Current.xml, Excluded.txt
/// — read by the daemon) under ~/.local/share/LittleBigMouse, settings under
/// ~/.config/LittleBigMouse. The Rust daemon resolves the same paths on its side.
/// </summary>
public static class LbmPaths
{
    public static string DataDir
        => OperatingSystem.IsWindows()
            ? Path.Combine(
                Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
                "Mgth", "LittleBigMouse")
            : Path.Combine(XdgHome("XDG_DATA_HOME", ".local/share"), "LittleBigMouse");

    public static string ConfigDir
        => OperatingSystem.IsWindows()
            ? DataDir
            : Path.Combine(XdgHome("XDG_CONFIG_HOME", ".config"), "LittleBigMouse");

    static string XdgHome(string variable, string fallback)
        => Environment.GetEnvironmentVariable(variable) is { Length: > 0 } dir
            ? dir
            : Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.UserProfile), fallback);
}
