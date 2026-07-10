#nullable enable
using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Text.Json;

namespace LittleBigMouse.Platform.Linux;

/// <summary>
/// Per-screen wallpaper through the plasmashell scripting DBus API — the one source
/// that gives both the screen's logical geometry (to match our monitors, kscreen
/// coordinates) and the org.kde.image configuration, without parsing appletsrc and
/// reverse-engineering the containment→screen mapping.
/// </summary>
public static class PlasmaWallpaper
{
    public record Entry(double X, double Y, double Width, double Height, string ImagePath, int FillMode);

    /// <summary>The config file plasma rewrites on wallpaper changes — watched for mtime.</summary>
    public static string ConfigPath => Path.Combine(
        Environment.GetEnvironmentVariable("XDG_CONFIG_HOME") is { Length: > 0 } c
            ? c
            : Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.UserProfile), ".config"),
        "plasma-org.kde.plasma.desktop-appletsrc");

    const string Script =
        "var out=[];" +
        "for(var i=0;i<screenCount;i++){" +
        "var g=screenGeometry(i);var d=desktopForScreen(i);" +
        "d.currentConfigGroup=[\"Wallpaper\",\"org.kde.image\",\"General\"];" +
        "out.push(g.x+\",\"+g.y+\",\"+g.width+\",\"+g.height+\"|\"+d.readConfig(\"FillMode\",2)+\"|\"+d.readConfig(\"Image\"));" +
        "}print(out.join(\"\\n\"));";

    public static List<Entry> Query()
    {
        var entries = new List<Entry>();

        string? reply;
        try
        {
            using var process = Process.Start(new ProcessStartInfo("busctl",
                $"--user --json=short call org.kde.plasmashell /PlasmaShell org.kde.PlasmaShell evaluateScript s \"{Script.Replace("\"", "\\\"")}\"")
            {
                RedirectStandardOutput = true,
                RedirectStandardError = true,
                UseShellExecute = false
            });
            if (process == null) return entries;
            reply = process.StandardOutput.ReadToEnd();
            process.WaitForExit(5000);
            if (process.ExitCode != 0) return entries;
        }
        catch
        {
            return entries;
        }

        try
        {
            using var doc = JsonDocument.Parse(reply);
            var payload = doc.RootElement.GetProperty("data")[0].GetString() ?? "";

            foreach (var line in payload.Split('\n', StringSplitOptions.RemoveEmptyEntries))
            {
                var parts = line.Split('|', 3);
                if (parts.Length < 3) continue;

                var geometry = parts[0].Split(',');
                if (geometry.Length != 4) continue;

                var image = ResolveImage(parts[2]);
                if (image == null) continue;

                entries.Add(new Entry(
                    double.Parse(geometry[0]),
                    double.Parse(geometry[1]),
                    double.Parse(geometry[2]),
                    double.Parse(geometry[3]),
                    image,
                    int.TryParse(parts[1], out var fill) ? fill : 2));
            }
        }
        catch
        {
            // malformed reply (plasma restarting...): no wallpapers this round
        }

        return entries;
    }

    /// <summary>
    /// org.kde.image stores either a plain image (file:// URI) or a wallpaper
    /// package directory; a package's images live under contents/images — pick
    /// the largest by pixel count encoded in the file name (e.g. 3840x2160.png).
    /// </summary>
    static string? ResolveImage(string uri)
    {
        var path = uri.StartsWith("file://", StringComparison.Ordinal) ? Uri.UnescapeDataString(uri[7..]) : uri;
        if (string.IsNullOrWhiteSpace(path)) return null;

        if (File.Exists(path)) return path;
        if (!Directory.Exists(path)) return null;

        var images = Path.Combine(path, "contents", "images");
        if (!Directory.Exists(images)) return null;

        return Directory.EnumerateFiles(images)
            .OrderByDescending(f =>
            {
                var name = Path.GetFileNameWithoutExtension(f).Split('x');
                return name.Length == 2 && long.TryParse(name[0], out var w) && long.TryParse(name[1], out var h)
                    ? w * h
                    : 0;
            })
            .FirstOrDefault();
    }
}
