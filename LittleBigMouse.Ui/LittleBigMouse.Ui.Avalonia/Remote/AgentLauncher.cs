#nullable enable
using System;
using System.Diagnostics;
using System.IO;
using System.Linq;

namespace LittleBigMouse.Ui.Avalonia.Remote;

/// <summary>
/// Starts the agent when nothing answers (v6, phase 4).
/// <para>
/// The agent normally comes up with the session — it owns an autostart entry of its own —
/// but nothing guarantees one is running when this window opens: a first run before
/// autostart has ever been written, a session where the user turned it off, an agent that
/// died. The window would then show a dead engine and refuse every button, with no way out
/// from inside it. So the frontend starts one, once, and lets its client connect.
/// </para>
/// <para>
/// It never stops one. An agent outlives the window that started it (that is the point of
/// the split), and quitting belongs to the tray's Exit.
/// </para>
/// </summary>
public sealed class AgentLauncher(Func<string?>? find = null, Func<string, bool>? start = null)
{
    readonly Func<string?> _find = find ?? Find;
    readonly Func<string, bool> _start = start ?? Start;

    public static string ExeName => OperatingSystem.IsWindows() ? "lbm-agent.exe" : "lbm-agent";

    /// <summary>Starts an agent. False when there was none to start, or it would not.</summary>
    public bool Launch()
    {
        if (_find() is not { } path)
        {
            Console.Error.WriteLine($"No agent answered and no {ExeName} was found to start.");
            return false;
        }
        if (!_start(path))
        {
            Console.Error.WriteLine($"No agent answered and {path} would not start.");
            return false;
        }
        Console.Error.WriteLine($"No agent answered: started {path}.");
        return true;
    }

    /// <summary>
    /// Where the agent is, without depending on the .NET target framework folder
    /// (net8.0, net9.0, …): installed, it sits next to this executable; in the dev tree it
    /// is a Cargo build under <c>rust/target</c>. Same probe the hook's had.
    /// </summary>
    public static string? Find()
    {
        var here = AppContext.BaseDirectory
            .TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar);

        var sibling = Path.Combine(here, ExeName);
        if (File.Exists(sibling)) return sibling;

        try
        {
            var project = Path.Combine("LittleBigMouse.Ui", "LittleBigMouse.Ui.Avalonia");
            var at = here.IndexOf(project, StringComparison.OrdinalIgnoreCase);
            if (at < 0) return null;

            // The build matching this one first, then whichever exists: a developer running
            // a Debug UI against a Release agent is working, not misconfigured.
            var separator = Path.DirectorySeparatorChar;
            var configuration = here.Contains($"{separator}Debug{separator}",
                StringComparison.OrdinalIgnoreCase)
                ? "debug"
                : "release";
            var target = Path.Combine(here[..at], "rust", "target");
            return new[]
                {
                    Path.Combine(target, configuration, ExeName),
                    Path.Combine(target, "release", ExeName),
                    Path.Combine(target, "debug", ExeName),
                }
                .FirstOrDefault(File.Exists);
        }
        catch (Exception error) when (error is IOException or UnauthorizedAccessException
                                      or ArgumentException)
        {
            return null;
        }
    }

    /// <summary>
    /// Starts it and lets go. No window, and no handle kept: this process is not the
    /// agent's owner — closing the window must leave it running.
    /// </summary>
    static bool Start(string path)
    {
        try
        {
            using var started = Process.Start(new ProcessStartInfo(path)
            {
                UseShellExecute = false,
                CreateNoWindow = true,
                WorkingDirectory = Path.GetDirectoryName(path) ?? ".",
            });
            return started is not null;
        }
        catch (Exception error) when (error is System.ComponentModel.Win32Exception
                                      or InvalidOperationException or IOException)
        {
            Console.Error.WriteLine($"Starting {path} failed: {error.Message}");
            return false;
        }
    }
}
