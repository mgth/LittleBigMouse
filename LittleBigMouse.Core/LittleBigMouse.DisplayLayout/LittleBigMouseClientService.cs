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
//using H.Pipes;
using LittleBigMouse.Zoning;

namespace LittleBigMouse.DisplayLayout;

public partial class LittleBigMouseClientService : ILittleBigMouseClientService
{
    public event EventHandler<LittleBigMouseServiceEventArgs> DaemonEventReceived;
    //NamedPipeClientStream _client;
    readonly RemoteClientSocket _client = new("localhost",25196);

    protected void OnStateChanged(LittleBigMouseEvent evt, string payload = "")
    {
        if(evt<=LittleBigMouseEvent.Dead)
        {
            State = evt;
        }
        DaemonEventReceived?.Invoke(this, new (evt,payload));
    }

    public LittleBigMouseClientService()
    {
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

    private void CreateExcludedFile()
    {
        string path = Environment.GetFolderPath(Environment.SpecialFolder.CommonApplicationData);
        var file = Path.Combine(path,"Mgth","LittleBigMouse","Excluded.txt");
        if(File.Exists(file))
        {
            // Riot games -> Riot Games (was misspelled in 5.0.4.0) TODO : remove in a version or two
            string text = File.ReadAllText(file);
            if(text.Contains("\\Riot games\\"))
            {
                text = text.Replace("\\Riot games\\", "\\Riot Games\\");
                File.WriteAllText(file, text);
            }
            // SteamLibrary -> steamapps is better for steam games TODO : remove in a version or two
            if(text.Contains("\\SteamLibrary\\"))
            {
                text = text.Replace("\\SteamLibrary\\", "\\steamapps\\");
                File.WriteAllText(file, text);
            }
            return;
        }
        File.WriteAllText(file, ":Excluded processes\n\\Epic Games\\\n\\steamapps\\\n\\Riot Games\\\n");
    }

    public void LaunchDaemon()
    {
        var processes = Process.GetProcessesByName("LittleBigMouse.Hook");
        foreach (var process in processes)
        {
            Debug.WriteLine($"Already running : {process.ProcessName} {process.Id}");
            return;
        }

        var path = Assembly.GetEntryAssembly()?.Location;
        if (path is null) return;

        if (path.Contains(@"\bin\"))
        {
            // .\LittleBigMouse.Ui.Avalonia\bin\x64\Debug\net8.0\LittleBigMouse.Ui.Avalonia.dll
            // .\x64\Debug\LittleBigMouse.Hook.exe

            path = path.Replace(@"\LittleBigMouse.Ui\LittleBigMouse.Ui.Avalonia\", @"\LittleBigMouse.Daemon\");
            path = path.Replace(@"\net8.0\", @"\");
            path = path.Replace(@"\net7.0\", @"\");
        }

        path = path.Replace(@"\LittleBigMouse.Ui.Avalonia.dll", @"\LittleBigMouse.Hook.exe");

        if (!File.Exists(path))
        {
            Debug.WriteLine($"Not found : {path}");
            return;
        }

        CreateExcludedFile();

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
        catch (ExecutionEngineException ex)
        {

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

        var data = Environment.GetFolderPath(Environment.SpecialFolder.CommonApplicationData);

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