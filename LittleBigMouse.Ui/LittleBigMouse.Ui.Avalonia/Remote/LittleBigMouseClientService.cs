using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Runtime.CompilerServices;
using System.Text.RegularExpressions;
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

    public LittleBigMouseClientService(ILayoutOptions options)
    {
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


    public Task StartAsync(ZonesLayout zonesLayout, CancellationToken token = default)
    {
        var commands = new List<CommandMessage>()
        {
            new(LittleBigMouseCommand.Load, zonesLayout),
            new(LittleBigMouseCommand.Run)
        };

        return SendMessagesAsync(commands, token);
    }


    public async Task StopAsync(CancellationToken token = default) => await SendAsync(token);

    public async Task QuitAsync(CancellationToken token = default) => await SendAsync(token);

    readonly SemaphoreSlim _startingSemaphore = new SemaphoreSlim(1, 1);

    Process? _daemonProcess;

    void CreateExcludedFile()
    {
        var path = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
        var dir = Path.Combine(path,"Mgth","LittleBigMouse");
        var file = Path.Combine(dir,"Excluded.txt");
        if(File.Exists(file)) return;

        Directory.CreateDirectory(dir);
        // Self-heal: a buggy earlier version created "Excluded.txt" as a *directory*.
        if (Directory.Exists(file)) Directory.Delete(file, true);
        File.WriteAllText(file, ":Excluded processes\n\\Epic Games\\\n\\steamapps\\\n\\Riot Games\\\n");
    }

    public void LaunchDaemon()
    {
        var processes = Process.GetProcessesByName("LittleBigMouse.Hook");
        foreach (var process in processes)
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

    const string HookExeName = "LittleBigMouse.Hook.exe";

    /// <summary>
    /// Locate LittleBigMouse.Hook.exe without depending on the .NET target framework folder
    /// (net8.0, net9.0, net10.0, ...). Deployed builds keep the hook next to the UI; in the dev
    /// tree the C++ hook is built under LittleBigMouse.Hook\bin with its own platform/config
    /// layout and no TFM subfolder, so we search that bin folder. Resistant to .NET version,
    /// platform (AnyCPU/x64) and configuration (Debug/Release) changes.
    /// </summary>
    static string? FindHookPath()
    {
        var uiDir = AppContext.BaseDirectory.TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar);

        // 1. Deployed / published build: the hook sits right next to the UI.
        var sibling = Path.Combine(uiDir, HookExeName);
        if (File.Exists(sibling)) return sibling;

        // 2. Dev tree: find the hook project's bin folder and search it.
        try
        {
            var projectSegment = Path.Combine("LittleBigMouse.Ui", "LittleBigMouse.Ui.Avalonia");
            var i = uiDir.IndexOf(projectSegment, StringComparison.OrdinalIgnoreCase);
            if (i < 0) return null;

            var hookBin = Path.Combine(uiDir[..i], "LittleBigMouse.Hook", "bin");
            if (!Directory.Exists(hookBin)) return null;

            // Prefer the build matching the UI's current configuration.
            var config = uiDir.Contains(@"\Debug\", StringComparison.OrdinalIgnoreCase) ? "Debug" : "Release";

            return Directory.EnumerateFiles(hookBin, HookExeName, SearchOption.AllDirectories)
                .OrderByDescending(p => p.Contains($@"\{config}\", StringComparison.OrdinalIgnoreCase))
                .FirstOrDefault();
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

        var data = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);

        var path = Path.Combine(
            data,
            @"Mgth\LittleBigMouse\Current.xml");

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