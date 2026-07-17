#nullable enable
using System;
using System.IO;
using System.Linq;
using System.Text.Json;
using Avalonia;
using Avalonia.Controls;
using LittleBigMouse.Plugins;

namespace LittleBigMouse.Ui.Avalonia.Main;

/// <summary>
/// Saves and restores the main window geometry across sessions (window.json in the
/// config dir — kept out of options.json: this is UI state, not a user setting).
/// On Wayland the position setter is a compositor no-op, so only the size is
/// effectively restored there; X11/XWayland restores both.
/// </summary>
public class MainWindowGeometry
{
    public int X { get; set; }
    public int Y { get; set; }
    public double Width { get; set; }
    public double Height { get; set; }
    public bool Maximized { get; set; }

    const double DefaultWidth = 1200;
    const double DefaultHeight = 700;

    static string FilePath => Path.Combine(LbmPaths.ConfigDir, "window.json");

    static readonly JsonSerializerOptions JsonOptions = new() { WriteIndented = true };

    /// <summary>Restore saved geometry onto the window and save it back on close.</summary>
    public static void Attach(Window window)
    {
        Restore(window);
        window.Closing += (_, _) => Save(window);
    }

    static void Restore(Window window)
    {
        // Sanity floor: a corrupt or hand-edited file must not restore an unusable window.
        if (Load() is { Width: >= 200, Height: >= 200 } geometry)
        {
            window.Width = geometry.Width;
            window.Height = geometry.Height;

            // Monitors may have changed since last run (this app's whole purpose):
            // only restore a position still visible on a current screen, otherwise
            // let the window manager place us.
            var rect = new PixelRect(geometry.X, geometry.Y, (int)geometry.Width, (int)geometry.Height);
            if (window.Screens.All.Any(s => s.WorkingArea.Intersects(rect)))
                window.Position = new PixelPoint(geometry.X, geometry.Y);
            else
                window.WindowStartupLocation = WindowStartupLocation.CenterScreen;

            if (geometry.Maximized)
                window.WindowState = WindowState.Maximized;
        }
        else
        {
            window.Width = DefaultWidth;
            window.Height = DefaultHeight;
            window.WindowStartupLocation = WindowStartupLocation.CenterScreen;
        }
    }

    static void Save(Window window)
    {
        // When closing maximized or minimized, Position/ClientSize describe that state,
        // not the normal one: keep the previously saved normal geometry and only track
        // the maximized flag.
        var geometry = window.WindowState == WindowState.Normal
            ? new MainWindowGeometry
            {
                X = window.Position.X,
                Y = window.Position.Y,
                Width = window.ClientSize.Width,
                Height = window.ClientSize.Height,
            }
            : Load() ?? new MainWindowGeometry { Width = DefaultWidth, Height = DefaultHeight };

        geometry.Maximized = window.WindowState == WindowState.Maximized;

        try
        {
            Directory.CreateDirectory(LbmPaths.ConfigDir);
            var temp = FilePath + ".tmp";
            File.WriteAllText(temp, JsonSerializer.Serialize(geometry, JsonOptions));
            File.Move(temp, FilePath, overwrite: true);
        }
        catch
        {
            // Best effort: losing the geometry must never prevent the window from closing.
        }
    }

    static MainWindowGeometry? Load()
    {
        try
        {
            var path = FilePath;
            if (!File.Exists(path)) return null;
            return JsonSerializer.Deserialize<MainWindowGeometry>(File.ReadAllText(path), JsonOptions);
        }
        catch
        {
            return null;
        }
    }
}
