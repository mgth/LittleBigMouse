using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Plugins;

namespace LittleBigMouse.Ui.Avalonia.Remote;

/// <summary>
/// Owns the hook daemon <em>process</em>: locating its executable, launching it (at most one),
/// force-stopping the one this UI started, and seeding the exclusion file the daemon reads.
/// Pulled out of <see cref="LittleBigMouseClientService"/> so the process ownership rules and the
/// stop/relaunch behaviour live in one testable place — the IPC transport and the recovery file
/// are separate concerns (<see cref="LocalIpcClient"/>, <see cref="RecoveryStateStore"/>).
/// <para>
/// <b>Process ownership.</b> The single daemon this manager launches is held in
/// <see cref="_daemonProcess"/> and owned for the manager's lifetime: it is disposed either when
/// replaced by a newer launch or when the manager itself is disposed. Handles obtained by
/// <see cref="IProcessHost.Enumerate"/> (foreign or otherwise) are short-lived and disposed as
/// soon as they are inspected — never retained. On Unix the manager will only ever stop the
/// instance it launched (enumerating every <c>lbm-hook</c> could target another user's process,
/// especially if the UI runs as root); on Windows it may stop any hook image in its own logon
/// session as a last resort.
/// </para>
/// <para>
/// <b>Disposal.</b> <see cref="Dispose"/> releases the owned launch handle only. It does NOT kill
/// the daemon: a Stop/Quit is an explicit user action routed through <see cref="StopCurrentSessionDaemons"/>,
/// and simply disposing the UI must leave a running daemon alone (autostart owns it thereafter).
/// </para>
/// </summary>
public sealed class DaemonProcessManager : IDisposable
{
    readonly IProcessHost _host;
    readonly Action _seedExcludedFile;
    readonly Func<string?> _resolveHookPath;
    readonly object _launchGate = new();
    IDaemonProcess? _daemonProcess;

    public DaemonProcessManager() : this(new SystemProcessHost())
    {
    }

    /// <param name="host">The OS process seam.</param>
    /// <param name="seedExcludedFile">
    /// Seeds the exclusion file the daemon reads (see <see cref="CreateExcludedFile"/>). Injectable
    /// so tests can supply a no-op and stay off the real per-user data directory; defaults to the
    /// production writer.
    /// </param>
    /// <param name="resolveHookPath">
    /// Locates the hook executable (see <see cref="FindHookPath"/>). Injectable so tests can return
    /// a fixed path without the daemon binary being present; defaults to the real probe.
    /// </param>
    public DaemonProcessManager(IProcessHost host, Action? seedExcludedFile = null,
        Func<string?>? resolveHookPath = null)
    {
        _host = host;
        _seedExcludedFile = seedExcludedFile ?? CreateExcludedFile;
        _resolveHookPath = resolveHookPath ?? FindHookPath;
    }

    /// <summary>
    /// Launch the daemon unless one is already running (in this session, on Windows). The
    /// check-and-launch sequence is atomic: the IPC listener and a foreground command can both
    /// observe a missing endpoint, and must not start two daemons.
    /// </summary>
    public void LaunchDaemon()
    {
        lock (_launchGate)
        {
            if (IsDaemonRunning()) return;

            var path = _resolveHookPath();
            if (path is null)
            {
                Debug.WriteLine($"Not found : {HookExeName}");
                return;
            }

            // Must not abort the daemon launch if the exclusion file can't be written.
            try { _seedExcludedFile(); }
            catch (Exception ex) { Debug.WriteLine($"CreateExcludedFile failed: {ex.Message}"); }

            try
            {
                var process = _host.Launch(path);
                _daemonProcess?.Dispose();
                _daemonProcess = process;
                Debug.WriteLine($"Started : {process.ProcessName} {process.Id}");
            }
            catch (Exception ex)
            {
                Debug.WriteLine($"LaunchDaemon failed: {ex}");
            }
        }
    }

    bool IsDaemonRunning()
    {
        foreach (var process in _host.Enumerate(HookProcessNames))
        {
            using (process)
            {
                if (process.HasExited) continue;
                if (OperatingSystem.IsWindows()
                    && process.SessionId != _host.CurrentSessionId) continue;
                Debug.WriteLine($"Already running : {process.ProcessName} {process.Id}");
                return true;
            }
        }
        return false;
    }

    /// <summary>
    /// Force-stop the daemon as a last resort when IPC could not deliver a clean Stop.
    /// </summary>
    /// <returns>True if no hook daemon is left running afterwards.</returns>
    public bool StopCurrentSessionDaemons()
    {
        // On Unix, process names are not scoped to a Windows-style logon session. Only terminate
        // the daemon instance this UI actually launched; enumerating every "lbm-hook" could
        // otherwise target another user's process (especially if the UI itself is running as root).
        if (!OperatingSystem.IsWindows())
        {
            if (_daemonProcess is not null) return _daemonProcess.TryStop();

            var daemonFound = false;
            foreach (var process in _host.Enumerate([HookProcessNames[0]]))
            {
                using (process)
                    daemonFound |= !process.HasExited;
            }
            return !daemonFound;
        }

        var stopped = true;
        var session = _host.CurrentSessionId;
        foreach (var process in _host.Enumerate(HookProcessNames))
        {
            using (process)
            {
                if (process.HasExited || process.SessionId != session) continue;
                if (!process.TryStop()) stopped = false;
            }
        }
        return stopped;
    }

    /// <summary>
    /// Last-resort seed of the exclusion list, right before the daemon starts reading it.
    /// <see cref="LittleBigMouse.Plugins.Persistence.ExcludedListPersistence"/> normally gets
    /// there first (loading a layout precedes launching the daemon) and writes the same content,
    /// header included; this stays as the guard for any path that reaches the daemon without a
    /// load, where the alternative is a daemon running with no exclusions at all. Both must keep
    /// writing the same thing.
    /// </summary>
    static void CreateExcludedFile()
    {
        var dir = LbmPaths.DataDir;
        var file = Path.Combine(dir, "Excluded.txt");
        if (File.Exists(file)) return;

        Directory.CreateDirectory(dir);
        // Self-heal: a buggy earlier version created "Excluded.txt" as a *directory*.
        if (Directory.Exists(file)) Directory.Delete(file, true);
        var lines = new[] { ExcludedProcessDefaults.Header }.Concat(ExcludedProcessDefaults.All);
        File.WriteAllText(file, string.Join("\n", lines) + "\n");
    }

    // Deployed Windows builds keep the historical staging name (CI renames the Rust binary to it);
    // a dev-tree Rust daemon runs under its cargo binary name on every platform.
    static string HookExeName => OperatingSystem.IsWindows() ? "LittleBigMouse.Hook.exe" : "lbm-hook";

    static string[] HookProcessNames => OperatingSystem.IsWindows()
        ? ["LittleBigMouse.Hook", "lbm-hook"]
        : ["lbm-hook"];

    /// <summary>
    /// Locate the hook daemon without depending on the .NET target framework folder
    /// (net8.0, net9.0, net10.0, ...). Deployed builds keep the hook next to the UI; in the dev
    /// tree the Rust hook is built under LittleBigMouse-Hook-Rust/target.
    /// Resistant to .NET version, platform (AnyCPU/x64) and configuration (Debug/Release) changes.
    /// </summary>
    static string? FindHookPath()
    {
        var uiDir = AppContext.BaseDirectory.TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar);

        // 1. Deployed / published build: the hook sits right next to the UI.
        var sibling = Path.Combine(uiDir, HookExeName);
        if (File.Exists(sibling)) return sibling;

        // 2. Dev tree: find the hook build output and search it.
        try
        {
            var projectSegment = Path.Combine("LittleBigMouse.Ui", "LittleBigMouse.Ui.Avalonia");
            var i = uiDir.IndexOf(projectSegment, StringComparison.OrdinalIgnoreCase);
            if (i < 0) return null;
            var root = uiDir[..i];

            // Prefer the build matching the UI's current configuration.
            var sep = Path.DirectorySeparatorChar;
            var config = uiDir.Contains($"{sep}Debug{sep}", StringComparison.OrdinalIgnoreCase) ? "Debug" : "Release";

            // Rust daemon first: LittleBigMouse-Hook-Rust/target/{debug,release}/lbm-hook[.exe].
            var rustExe = OperatingSystem.IsWindows() ? "lbm-hook.exe" : "lbm-hook";
            var target = Path.Combine(root, "LittleBigMouse-Hook-Rust", "target");
            var rust = new[]
                {
                    Path.Combine(target, config.ToLowerInvariant(), rustExe),
                    Path.Combine(target, "release", rustExe),
                    Path.Combine(target, "debug", rustExe),
                }
                .FirstOrDefault(File.Exists);
            return rust;
        }
        catch
        {
            return null;
        }
    }

    /// <summary>
    /// Release the owned launch handle. Does not stop the daemon — see the type remarks.
    /// </summary>
    public void Dispose()
    {
        _daemonProcess?.Dispose();
        _daemonProcess = null;
    }
}
