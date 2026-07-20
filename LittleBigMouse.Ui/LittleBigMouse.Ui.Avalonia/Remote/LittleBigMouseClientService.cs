using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Runtime.CompilerServices;
using System.Text.RegularExpressions;
using LittleBigMouse.Plugins;
using System.Threading;
using System.Threading.Tasks;
using HLab.Remote;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Zoning;

namespace LittleBigMouse.Ui.Avalonia.Remote;

public partial class LittleBigMouseClientService : ILittleBigMouseClientService
{
    public event EventHandler<LittleBigMouseServiceEventArgs>? DaemonEventReceived;
    //NamedPipeClientStream _client;
    readonly RemoteClientSocket _client;



    protected void OnStateChanged(LittleBigMouseEvent evt, string payload = "")
    {
        if(evt<=LittleBigMouseEvent.Dead)
        {
            State = evt;
        }
        DaemonEventReceived?.Invoke(this, new (evt,payload));
    }

    readonly IDisplayController _displayController;

    public LittleBigMouseClientService(ILayoutOptions options, IDisplayController displayController)
    {
        _displayController = displayController;
        _client = new("localhost", options.DaemonPort);

        _client.ConnectionFailed += (sender, args) =>
        {
            Debug.WriteLine($"ConnectionFailed : Launch daemon");
            LaunchDaemon();
        };

        _client.MessageReceived += (sender, args) =>
        {
            //TODO : message interpretation too lazy
            if(args.Contains("Stopped"))
                OnStateChanged(LittleBigMouseEvent.Stopped);
            else if(args.Contains("Running"))
                OnStateChanged(LittleBigMouseEvent.Running);
            else if(args.Contains("Dead"))
                OnStateChanged(LittleBigMouseEvent.Dead);
            else if(args.Contains("DisplayChanged"))
                OnStateChanged(LittleBigMouseEvent.DisplayChanged);
            else if(args.Contains("DesktopChanged"))
                OnStateChanged(LittleBigMouseEvent.DesktopChanged);
            else if(args.Contains("Suspended"))
                OnStateChanged(LittleBigMouseEvent.Suspended);
            else if(args.Contains("Resumed"))
                OnStateChanged(LittleBigMouseEvent.Resumed);
            else if(args.Contains("FocusChanged"))
            {
                var payload = PayloadRegex().Match(args).Groups[1].Value;
                OnStateChanged(LittleBigMouseEvent.FocusChanged,payload);
            }
        };

        _client.Connected += (sender, args) =>
        {
            Debug.WriteLine($"Connected");
            OnStateChanged(LittleBigMouseEvent.Connected);
        };

        _client.Listen();
    }

    public LittleBigMouseEvent State { get; private set; }


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
        await SendAsync(token);
        await Task.Run(_displayController.RestoreAfterEngine, token);
    }

    public async Task QuitAsync(CancellationToken token = default)
    {
        await SendAsync(token);
        await Task.Run(_displayController.RestoreAfterEngine, token);
    }

    readonly SemaphoreSlim _startingSemaphore = new SemaphoreSlim(1, 1);

    Process? _daemonProcess;

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
        foreach (var name in HookProcessNames)
        foreach (var process in Process.GetProcessesByName(name))
        {
            if(process.HasExited) continue;
            Debug.WriteLine($"Already running : {process.ProcessName} {process.Id}");
            return;
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
            var startInfo = new ProcessStartInfo
            {
                FileName = path,

                //RedirectStandardOutput = true,
                //RedirectStandardError = true,

                #if DEBUG
                UseShellExecute = true,
                #else
                UseShellExecute = false,
                CreateNoWindow = true,
                #endif

            };

            var process = new Process { StartInfo = startInfo};

            process.Start();

            _daemonProcess = process;

            Debug.WriteLine($"Started : {process.ProcessName} {process.Id}");
        }
        catch (Exception ex)
        {
            Debug.WriteLine($"LaunchDaemon failed: {ex}");
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
        return SendMessagesAsync([message], token);
    }

    async Task SendMessagesAsync(IEnumerable<CommandMessage> messages, CancellationToken token = default)
    {
        var xml = messages.Aggregate("", (current, command) => current + $"{command.Serialize()}\n");

        // The daemon reads this file back on startup: LbmPaths must match its side
        // (%LOCALAPPDATA%\Mgth\LittleBigMouse on Windows, ~/.local/share/LittleBigMouse on
        // Linux). The former literal @"Mgth\LittleBigMouse\Current.xml" produced a file
        // NAMED with backslashes on Linux.
        var path = Path.Combine(LbmPaths.DataDir, "Current.xml");

        if (!Directory.Exists(Path.GetDirectoryName(path)))
            Directory.CreateDirectory(Path.GetDirectoryName(path));

        try
        {
            await using var outputFile = new StreamWriter(path, false);
            await outputFile.WriteAsync(xml);
        }
        catch { }


        //byte[] messageBytes = Encoding.UTF8.GetBytes(xml);

        await _client.SendMessageAsync(xml, token); //.StandardInput.WriteAsync(xml);
    }

    [GeneratedRegex("<Payload>(.*)</Payload>")]
    private static partial Regex PayloadRegex();
}
