#nullable enable
using System;
using System.IO;
using System.Reflection;
using System.Runtime.InteropServices;
using LittleBigMouse.Plugins;

namespace LittleBigMouse.Ui.Avalonia;

/// <summary>
/// Windows runs the UI as a WinExe: without a console every Console.Error write
/// (boot failures, unhandled dispatcher exceptions, ViewLocator diagnostics…) is
/// silently discarded — "the app does nothing and there are no logs anywhere"
/// (#448, #510). When stderr has no backing handle, console output is routed to
/// %LOCALAPPDATA%\Mgth\LittleBigMouse\ui.log instead (the previous runs kept as
/// ui.prev.log, ui.prev.2.log … see <see cref="LogRotation"/>). A real console or an
/// explicit `2&gt; file` redirection provides a
/// valid handle and wins over the file. Linux keeps plain stderr (run-lbm.sh and
/// the desktop session already capture it).
/// </summary>
internal static class StartupLog
{
    const int STD_ERROR_HANDLE = -12;

    /// <summary>
    /// Where console output goes while detached; null when a real console or a redirection
    /// has it. What the boot failure window points the user to.
    /// </summary>
    public static string? LogFile { get; private set; }

    [DllImport("kernel32.dll")]
    static extern IntPtr GetStdHandle(int nStdHandle);

    public static void RedirectConsoleWhenDetached()
    {
        if (!OperatingSystem.IsWindows()) return;

        try
        {
            var stderr = GetStdHandle(STD_ERROR_HANDLE);
            if (stderr != IntPtr.Zero && stderr != new IntPtr(-1)) return;

            Directory.CreateDirectory(LbmPaths.DataDir);
            var path = Path.Combine(LbmPaths.DataDir, "ui.log");

            // Throws while another instance holds ui.log open: that instance owns
            // the log and this one is about to exit on the single-instance guard.
            LogRotation.Rotate(path);

            var writer = new StreamWriter(
                new FileStream(path, FileMode.Create, FileAccess.Write, FileShare.Read))
            { AutoFlush = true };

            Console.SetOut(writer);
            Console.SetError(writer);
            LogFile = path;

            var version = Assembly.GetExecutingAssembly()
                .GetCustomAttribute<AssemblyInformationalVersionAttribute>()?.InformationalVersion ?? "?";
            Console.Error.WriteLine(
                $"[{DateTime.Now:yyyy-MM-dd HH:mm:ss}] LittleBigMouse {version} starting"
                + $" (pid {Environment.ProcessId}, {RuntimeInformation.OSDescription})");
        }
        catch
        {
            // Logging must never keep the app from starting.
        }
    }
}
