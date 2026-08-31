#nullable enable
using System;
using System.IO;
using Microsoft.Win32;

namespace LittleBigMouse.Platform.Windows;

/// <summary>
/// The immutable snapshot of the OS-level desktop wallpaper state read from the registry
/// and the transcoded-wallpaper file. This is what <see cref="WindowsWallpaperReader"/>
/// returns: the raw facts, decoupled from the reactive model and from COM.
/// <para>
/// <see cref="ImagePath"/> is the current wallpaper image path (empty for a solid color),
/// used as the reliable fallback while <c>IDesktopWallpaper.GetWallpaper</c> returns "" during
/// the wallpaper fade. <see cref="Signature"/> is the cheap change-detection fingerprint the UI
/// polls: equal signatures mean nothing changed, so the image is not re-read or re-processed.
/// </para>
/// </summary>
internal readonly record struct WindowsWallpaperState(string ImagePath, string Signature)
{
    /// <summary>The state read when nothing could be obtained (off Windows, or on any failure).</summary>
    public static readonly WindowsWallpaperState Empty = new("", "");
}

/// <summary>
/// The one place that reads Windows wallpaper details from the registry
/// (<c>HKCU\Control Panel\Desktop</c>, <c>HKCU\Control Panel\Colors</c>) and the transcoded
/// wallpaper file. Every handle it opens is released before it returns (the registry keys via
/// <c>using</c>; the file is only stat'd, never opened), and it never throws — any failure
/// yields <see cref="WindowsWallpaperState.Empty"/>.
/// <para>
/// Split out of <see cref="WindowsWallpaperMapping"/> so the OS-poking detail (key names, value
/// encoding, the fade-window fallback and the poll signature) lives behind one narrow, immutable
/// seam: the mapping only copies the resulting facts into the reactive model, and the signature
/// gate in <see cref="WindowsLayoutFactory"/> reads the same fingerprint this reader computes.
/// </para>
/// </summary>
internal static class WindowsWallpaperReader
{
    const string DesktopKey = @"Control Panel\Desktop";
    const string ColorsKey = @"Control Panel\Colors";

    /// <summary>
    /// Read the full wallpaper state in one pass: the current image path plus the poll signature.
    /// Prefer this over calling <see cref="ReadImagePath"/> and <see cref="ReadSignature"/>
    /// separately when both are needed, so the Desktop key is opened once.
    /// </summary>
    public static WindowsWallpaperState Read()
    {
        if (!OperatingSystem.IsWindows()) return WindowsWallpaperState.Empty;

        try
        {
            using var desktop = Registry.CurrentUser.OpenSubKey(DesktopKey);
            using var colors = Registry.CurrentUser.OpenSubKey(ColorsKey);

            var image = StringValue(desktop, "WallPaper");
            return new WindowsWallpaperState(image, ComposeSignature(desktop, colors));
        }
        catch
        {
            // Registry/file access can fail (locked hive, missing key, permissions): the caller
            // treats Empty as "unchanged / no override", never as a real "solid color" state.
            return WindowsWallpaperState.Empty;
        }
    }

    /// <summary>
    /// The current desktop wallpaper image path from <c>HKCU\Control Panel\Desktop\WallPaper</c>
    /// (empty for a solid color). Written immediately on change, unlike
    /// <c>IDesktopWallpaper.GetWallpaper</c> which returns "" during the wallpaper fade — a reliable
    /// fallback for a live change. Kept as a focused entry point for callers that only need the path.
    /// </summary>
    public static string ReadImagePath()
    {
        if (!OperatingSystem.IsWindows()) return "";

        try
        {
            using var desktop = Registry.CurrentUser.OpenSubKey(DesktopKey);
            return StringValue(desktop, "WallPaper");
        }
        catch
        {
            return "";
        }
    }

    /// <summary>
    /// A cheap fingerprint of the current desktop wallpaper state — registry values plus the
    /// transcoded-wallpaper file timestamp (rewritten on every image change). Changes whenever the
    /// image, fit or background color changes, so callers can poll it to detect a wallpaper change
    /// without touching the display device tree or COM.
    /// </summary>
    public static string ReadSignature()
    {
        if (!OperatingSystem.IsWindows()) return "";

        try
        {
            using var desktop = Registry.CurrentUser.OpenSubKey(DesktopKey);
            using var colors = Registry.CurrentUser.OpenSubKey(ColorsKey);
            return ComposeSignature(desktop, colors);
        }
        catch
        {
            return "";
        }
    }

    /// <summary>
    /// Build the poll signature from already-opened keys. The format is
    /// <c>wallpaper|style|tile|background|transcoded-mtime</c>; any missing value contributes an
    /// empty field, so two reads of the same OS state always compare equal (stable signature).
    /// </summary>
    static string ComposeSignature(RegistryKey? desktop, RegistryKey? colors)
    {
        var wp = StringValue(desktop, "WallPaper");
        var style = ToStringValue(desktop, "WallpaperStyle");
        var tile = ToStringValue(desktop, "TileWallpaper");
        var background = ToStringValue(colors, "Background");
        var mtime = TranscodedWallpaperTicks();

        return $"{wp}|{style}|{tile}|{background}|{mtime}";
    }

    /// <summary>
    /// Last-write time (UTC ticks) of the per-user TranscodedWallpaper cache file, or 0 when it is
    /// absent. Windows rewrites this file on every image change, so it moves the signature even when
    /// the WallPaper path string is reused (same file re-selected, slideshow next image).
    /// </summary>
    static long TranscodedWallpaperTicks()
    {
        var transcoded = Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
            "Microsoft", "Windows", "Themes", "TranscodedWallpaper");

        return File.Exists(transcoded) ? File.GetLastWriteTimeUtc(transcoded).Ticks : 0L;
    }

    static string StringValue(RegistryKey? key, string name)
        => key?.GetValue(name) as string ?? "";

    static string ToStringValue(RegistryKey? key, string name)
        => key?.GetValue(name)?.ToString() ?? "";
}
