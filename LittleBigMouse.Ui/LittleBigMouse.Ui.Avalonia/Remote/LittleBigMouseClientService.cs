using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Runtime.CompilerServices;
using LittleBigMouse.Plugins;
using System.Threading;
using System.Threading.Tasks;
using System.Xml;
using Avalonia.Threading;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Zoning;

namespace LittleBigMouse.Ui.Avalonia.Remote;

public class LittleBigMouseClientService : ILittleBigMouseClientService, IDisposable
{
    public event EventHandler<LittleBigMouseServiceEventArgs>? DaemonEventReceived;
    readonly LocalIpcClient _client;



    protected void OnStateChanged(LittleBigMouseEvent evt, string payload = "")
    {
        if(evt<=LittleBigMouseEvent.Dead)
        {
            State = evt;
        }
        DaemonEventReceived?.Invoke(this, new (evt,payload));
    }

    readonly IDisplayController _displayController;
    readonly ILayoutOptions _options;

    public LittleBigMouseClientService(ILayoutOptions options, IDisplayController displayController)
    {
        _displayController = displayController;
        _options = options;
        _client = new LocalIpcClient();

        _client.ConnectionFailed += (sender, args) =>
        {
            Debug.WriteLine($"ConnectionFailed : Launch daemon");
            LaunchDaemon();
        };

        _client.MessageReceived += (sender, args) =>
        {
            if (!DaemonMessage.TryParse(args, out var message)) return;
            Dispatcher.UIThread.Post(() => OnStateChanged(message.Event, message.Payload));
        };

        _client.Connected += (sender, args) =>
        {
            Debug.WriteLine($"Connected");
            Dispatcher.UIThread.Post(() => OnStateChanged(LittleBigMouseEvent.Connected));
        };

        _client.Listen();
    }

    public LittleBigMouseEvent State { get; private set; } = LittleBigMouseEvent.Dead;


    public async Task StartAsync(ZonesLayout zonesLayout, CancellationToken token = default)
    {
        // Topology prologue (Linux/KWin: open 1px gaps so the daemon's barriers pass the
        // compositor validator; Windows: no-op, returns false). When it actually moves
        // outputs, the zones we were handed are stale by construction (computed in the
        // pre-gap space) — sending them would race the fresh ones and could win, arming
        // barriers in the wrong coordinate space. Drop this send: PrepareForEngine raised
        // DisplayChanged, MainService rebuilds and re-enters here with zones computed in
        // the gapped space (every start path runs with Options.Enabled set, which gates
        // that re-entry). Subprocess work: keep it off the UI thread.
        if (await Task.Run(_displayController.PrepareForEngine, token))
            return;

        var commands = new List<CommandMessage>()
        {
            new(LittleBigMouseCommand.Load, zonesLayout),
            new(LittleBigMouseCommand.Run)
        };

        await SendMessagesAsync(commands, token);
    }


    // The topology epilogue runs on explicit Stop/Quit only — NOT on a Dead daemon: the
    // socket layer auto-relaunches the daemon and MainService auto-restarts the engine
    // (Connected→Stopped→Start), so restoring there would fight the recovery. A daemon
    // that stays dead is covered by RecoverStale at next startup.
    public async Task StopAsync(CancellationToken token = default)
    {
        try
        {
            await SendAsync(token);
        }
        catch (Exception error) when (error is IOException
                                      or OperationCanceledException
                                      or UnauthorizedAccessException)
        {
            // Stop is a safety operation: losing IPC must not leave the input
            // hook active or fault ReactiveCommand's observable pipeline. Kill
            // only known hook images in this logon session as the last resort.
            var stopped = ForceStopCurrentSessionDaemons();
            Debug.WriteLine($"Stop IPC failed; daemon fallback stopped={stopped}: {error}");
            OnStateChanged(stopped ? LittleBigMouseEvent.Stopped : LittleBigMouseEvent.Dead);
        }
        await Task.Run(_displayController.RestoreAfterEngine, token);
    }

    public async Task QuitAsync(CancellationToken token = default)
    {
        await SendAsync(token);
        await Task.Run(_displayController.RestoreAfterEngine, token);
    }

    readonly object _daemonLaunchGate = new();
    Process? _daemonProcess;

    bool ForceStopCurrentSessionDaemons()
    {
        // On Unix, process names are not scoped to a Windows-style logon session.
        // Only terminate the daemon instance this UI actually launched; enumerating
        // every "lbm-hook" could otherwise target another user's process (especially
        // if the UI itself is running as root).
        if (!OperatingSystem.IsWindows())
        {
            if (_daemonProcess is not null) return TryStopProcess(_daemonProcess);

            var daemonFound = false;
            foreach (var process in Process.GetProcessesByName(HookProcessNames[0]))
            {
                using (process)
                    daemonFound |= !process.HasExited;
            }
            return !daemonFound;
        }

        var stopped = true;
        var session = Process.GetCurrentProcess().SessionId;
        foreach (var name in HookProcessNames)
        foreach (var process in Process.GetProcessesByName(name))
        {
            using (process)
            {
                if (process.HasExited || process.SessionId != session) continue;
                if (!TryStopProcess(process)) stopped = false;
            }
        }
        return stopped;
    }

    static bool TryStopProcess(Process process)
    {
        try
        {
            if (process.HasExited) return true;
            process.Kill(entireProcessTree: true);
            return process.WaitForExit(2000);
        }
        catch (Exception error) when (error is InvalidOperationException
                                      or System.ComponentModel.Win32Exception
                                      or NotSupportedException)
        {
            Debug.WriteLine($"Could not force-stop {process.ProcessName}: {error.Message}");
            return false;
        }
    }

    void CreateExcludedFile()
    {
        var dir = LbmPaths.DataDir;
        var file = Path.Combine(dir,"Excluded.txt");
        if(File.Exists(file)) return;

        Directory.CreateDirectory(dir);
        // Self-heal: a buggy earlier version created "Excluded.txt" as a *directory*.
        if (Directory.Exists(file)) Directory.Delete(file, true);
        var lines = new[] { ExcludedProcessDefaults.Header }.Concat(ExcludedProcessDefaults.All);
        File.WriteAllText(file, string.Join("\n", lines) + "\n");
    }

    public void LaunchDaemon()
    {
        // The listener and a foreground command can both observe a missing endpoint.
        // Keep their check-and-launch sequence atomic so they cannot start two daemons.
        lock (_daemonLaunchGate)
        {
            foreach (var name in HookProcessNames)
            foreach (var process in Process.GetProcessesByName(name))
            {
                using (process)
                {
                    if(process.HasExited) continue;
                    if (OperatingSystem.IsWindows()
                        && process.SessionId != Process.GetCurrentProcess().SessionId) continue;
                    Debug.WriteLine($"Already running : {process.ProcessName} {process.Id}");
                    return;
                }
            }

            var path = FindHookPath();
            if (path is null)
            {
                Debug.WriteLine($"Not found : {HookExeName}");
                return;
            }

            // Must not abort the daemon launch if the exclusion file can't be written.
            try { CreateExcludedFile(); }
            catch (Exception ex) { Debug.WriteLine($"CreateExcludedFile failed: {ex.Message}"); }

            try
            {
                // The UI always remains at the user's integrity level. Elevation is
                // narrowly scoped to the mouse engine when the user explicitly asks
                // for transitions over elevated applications.
                var startInfo = DaemonLaunchPolicy.Create(path,
                    OperatingSystem.IsWindows() && _options.StartElevated);

                var process = new Process { StartInfo = startInfo};

                process.Start();

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

    // Deployed Windows builds keep the historical staging name (CI renames the Rust
    // binary to it); a dev-tree Rust daemon runs under its cargo binary name on
    // every platform.
    static string HookExeName => OperatingSystem.IsWindows() ? "LittleBigMouse.Hook.exe" : "lbm-hook";
    static string[] HookProcessNames => OperatingSystem.IsWindows()
        ? ["LittleBigMouse.Hook", "lbm-hook"]
        : ["lbm-hook"];

    /// <summary>
    /// Locate the hook daemon without depending on the .NET target framework folder
    /// (net8.0, net9.0, net10.0, ...). Deployed builds keep the hook next to the UI; in the
    /// dev tree the Rust hook is built under LittleBigMouse-Hook-Rust/target.
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

    async Task StopDaemon(CancellationToken token = default)
    {
        await SendMessageAsync(new CommandMessage(LittleBigMouseCommand.Stop,null), _timeout, token);
    }

    readonly int _timeout = 5000;

    Task SendAsync(CancellationToken token = default, [CallerMemberName]string name = null)
    {
        if(name==null) throw new ArgumentNullException(nameof(name));
        if (name.EndsWith("Async")) name = name[..^5];
        return Enum.TryParse<LittleBigMouseCommand>(name, out var command) ? SendMessageAsync(
            new CommandMessage(command,null),_timeout,token) : Task.CompletedTask;
    }

    Task SendMessageAsync(CommandMessage message, int timeout,
        CancellationToken token = default)
    {
        return SendMessagesAsync([message], token, timeout);
    }

    async Task SendMessagesAsync(IEnumerable<CommandMessage> messages,
        CancellationToken token = default, int timeout = 5000)
    {
        var commands = messages.ToList();
        var recoveryXml = string.Join("\n", commands.Select(command => command.Serialize())) + "\n";
        var wireXml = $"<Messages>{string.Concat(commands.Select(command => command.Serialize()))}</Messages>";

        // The daemon reads this file back on startup: LbmPaths must match its side
        // (%LOCALAPPDATA%\Mgth\LittleBigMouse on Windows, ~/.local/share/LittleBigMouse on
        // Linux). The former literal @"Mgth\LittleBigMouse\Current.xml" produced a file
        // NAMED with backslashes on Linux.
        var path = Path.Combine(LbmPaths.DataDir, "Current.xml");

        Exception? persistenceFailure = null;
        if (commands.Any(command => command.Command == LittleBigMouseCommand.Load))
        {
            try
            {
                await AtomicRecoveryFile.WriteAsync(path, recoveryXml, token);
            }
            catch (Exception error) when (error is IOException or UnauthorizedAccessException or XmlException)
            {
                persistenceFailure = error;
            }
        }

        await _client.SendMessageAsync(wireXml, TimeSpan.FromMilliseconds(timeout), token);
        if (persistenceFailure is not null)
            throw new InvalidOperationException(
                "The live configuration was applied, but crash-recovery settings could not be saved.",
                persistenceFailure);
    }

    public void Dispose()
    {
        _client.Dispose();
        _daemonProcess?.Dispose();
        GC.SuppressFinalize(this);
    }
}
