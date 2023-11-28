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

public class LittleBigMouseClientService : ILittleBigMouseClientService
{
    public event EventHandler<LittleBigMouseServiceEventArgs> DaemonEventReceived;
    //NamedPipeClientStream _client;
    RemoteClientSocket _client;

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
        Task.Run(()=>StartDaemonAsync());
     }

    public async void Start() => await SendAsync();

    public Task StartAsync(ZonesLayout zonesLayout, CancellationToken token = default)
    {

        var commands = new List<CommandMessage>()
        {
            new(LittleBigMouseCommand.Load, zonesLayout),
            new(LittleBigMouseCommand.Run)
        };

        return SendMessageWithStartAsync(commands, _timeout, token);
    }

    public Func<ZonesLayout> ZonesLayoutGetter { get; set; } = () => null;
    public LittleBigMouseEvent State { get; set; }

    public async Task StopAsync(CancellationToken token = default) => await SendAsync(token);

    public async Task QuitAsync(CancellationToken token = default) => await SendAsync(token);

    public async Task LoadAtStartupAsync(bool state = true) => await SendAsync();

    public async Task CommandLineAsync(IList<string> args, CancellationToken token = default) => await SendAsync(token);

    public async Task RunningAsync() => await SendAsync();

    readonly SemaphoreSlim _startingSemaphore = new SemaphoreSlim(1, 1);

    Process? _daemonProcess;

    void LaunchDaemon()
    {
        var processes = Process.GetProcessesByName("LittleBigMouse.Daemon");
        foreach (var process in processes)
        {
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

        if(!File.Exists(path)) return;

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

            _daemonProcess = process;//Process.Start(path);

            process.WaitForInputIdle();
        }
        catch (ExecutionEngineException ex)
        {

        }
    }

    async Task<bool> StartDaemonAsync(int timeout = 10000, CancellationToken token = default)
    {
        //await StopDaemon();

        await _startingSemaphore.WaitAsync(token);
        if (token.IsCancellationRequested) return false;
        try
        {
            if (_client != null)
            {
                if (_client.IsConnected) return true;
                _client = null;
            }
                
            //_client = new NamedPipeClientStream(".", "lbm-daemon", PipeDirection.Out);
            _client = new RemoteClientSocket("localhost",25196);

            //_client.ConnectionFailed += (sender, args) => LaunchDaemon();

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
                    var payload = Regex.Match(args,"<Payload>(.*)</Payload>").Groups[1].Value;
                    OnStateChanged(LittleBigMouseEvent.FocusChanged,payload);
                }
            };

            _client.Listen();

            return true;


        }
        catch (TimeoutException)
        {
            return false;
        }

        finally
        {
            _startingSemaphore.Release();
        }
    }


    async Task StopDaemon(CancellationToken token = default)
    {
        //TODO : Maybe we should not use WithStart here.
        await SendMessageWithStartAsync(new CommandMessage(LittleBigMouseCommand.Stop,null), _timeout, token);
    }

    void _daemonProcess_Exited(object sender, EventArgs e)
    {
        throw new NotImplementedException();
    }

    readonly int _timeout = 5000;

    Task SendAsync(CancellationToken token = default, [CallerMemberName]string name = null)
    {
        if(name==null) throw new ArgumentNullException(nameof(name));
        if (name.EndsWith("Async")) name = name[..^5];
        return Enum.TryParse<LittleBigMouseCommand>(name, out var command) ? SendMessageWithStartAsync(
            new CommandMessage(command,null),_timeout,token) : Task.CompletedTask;
    }

    Task SendMessageWithStartAsync(CommandMessage message, int timeout,
        CancellationToken token = default)
    {
        return SendMessageWithStartAsync(new List<CommandMessage>() {message}, timeout, token);
    }

    async Task SendMessageWithStartAsync(List<CommandMessage> messages, int timeout, CancellationToken token = default)
    {
        if (!await StartDaemonAsync(timeout, token)) return;

        var retry = true;
        while (retry)
        {
            try
            {
                await SendMessageAsync(messages, token);
                return;
            }
            catch (TimeoutException)
            {
                retry = await StartDaemonAsync(timeout, token);
            }
        }
    }

    async Task SendMessageAsync(List<CommandMessage> messages, CancellationToken token = default)
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


}