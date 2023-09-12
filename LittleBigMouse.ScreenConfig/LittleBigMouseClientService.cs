using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.IO.Pipes;
using System.Runtime.CompilerServices;
using System.Text;
using System.Threading;
using System.Threading.Tasks;
using System.Xml;
using System.Xml.Serialization;
using HLab.Remote;
//using H.Pipes;
using LittleBigMouse.Zoning;
using Newtonsoft.Json;
using static HLab.Sys.Windows.API.WinUser;

namespace LittleBigMouse.DisplayLayout
{
    public class LittleBigMouseClientService : ILittleBigMouseClientService
    {
        public event EventHandler<LittleBigMouseServiceEventArgs> StateChanged;
        //NamedPipeClientStream _client;
        RemoteClientSocket _client;

        protected void OnStateChanged(LittleBigMouseState state)
        {
            StateChanged?.Invoke(this, new (state));
        }

        public LittleBigMouseClientService()
        {
        }

        public async void Start() => await SendAsync();

        public async Task StartAsync(ZonesLayout layout, CancellationToken token = default)
        {
            await SendMessageWithStartAsync(new DaemonMessage(LittleBigMouseCommand.Load, layout), _timeout, token);
            await SendMessageWithStartAsync(new DaemonMessage(LittleBigMouseCommand.Run), _timeout, token);
        }

        public async Task StopAsync(CancellationToken token = default) => await SendAsync(token);

        public async Task QuitAsync(CancellationToken token = default) => await SendAsync(token);

        public async Task LoadAtStartupAsync(bool state = true) => await SendAsync();

        public async Task CommandLineAsync(IList<string> args, CancellationToken token = default) => await SendAsync(token);

        public async Task RunningAsync() => await SendAsync();

        readonly SemaphoreSlim _startingSemaphore = new SemaphoreSlim(1, 1);

        Process _daemonProcess;
        async Task<bool> StartDaemonAsync(int timeout, CancellationToken token = default)
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
                _client = new RemoteClientSocket();
                _client.MessageReceived += (sender, args) =>
                {
                    if(args.Contains("Stopped"))
                        OnStateChanged(LittleBigMouseState.Stopped);
                    else if(args.Contains("Running"))
                        OnStateChanged(LittleBigMouseState.Running);
                    else if(args.Contains("Dead"))
                        OnStateChanged(LittleBigMouseState.Dead);
                };
                _client.Listen();

                await _client.ConnectAsync(timeout, token);
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
            await SendMessageWithStartAsync(new DaemonMessage(LittleBigMouseCommand.Stop,null), _timeout, token);
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
                new DaemonMessage(command,null),_timeout,token) : Task.CompletedTask;
        }

        async Task SendMessageWithStartAsync(DaemonMessage message, int timeout, CancellationToken token = default)
        {
            if (await StartDaemonAsync(timeout,token))
            {
                var retry = true;
                while (retry)
                {
                    try
                    {
                        await SendMessageAsync(message, token);
                        return;
                    }
                    catch (TimeoutException)
                    {
                        retry = await StartDaemonAsync(timeout,token);
                    }

                }
            }
        }

        async Task SendMessageAsync(DaemonMessage message, CancellationToken token = default)
        {
             var xml = message.Serialize();

            byte[] messageBytes = Encoding.UTF8.GetBytes(xml);

            await _client.SendMessageAsync(xml, token); //.StandardInput.WriteAsync(xml);
        }


    }

}