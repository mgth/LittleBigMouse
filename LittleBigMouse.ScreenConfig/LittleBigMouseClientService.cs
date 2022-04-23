using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.IO.Pipes;
using System.Linq.Expressions;
using System.Runtime.CompilerServices;
using System.Runtime.Serialization;
using System.Threading;
using System.Threading.Tasks;
using System.Windows.Documents;
using System.Xml.Serialization;
using H.Pipes;
using H.Pipes.Args;
using HLab.Remote;
using LittleBigMouse.Zoning;
using Newtonsoft.Json;

namespace LittleBigMouse.DisplayLayout
{

    //[Export(typeof(ILittleBigMouseClientService)),Singleton]
    public class LittleBigMouseClientService : ILittleBigMouseClientService
    {
        public event EventHandler<LittleBigMouseServiceEventArgs> StateChanged;
        private PipeClient<DaemonMessage> _client;

        protected void OnStateChanged(LittleBigMouseState state)
        {
            StateChanged?.Invoke(this, new (state));
        }

        public LittleBigMouseClientService()
        {
        }

        public async void Start() => await SendAsync();

        public async void Start(ZonesLayout layout)
        {
            var message = new DaemonMessage(LittleBigMouseCommand.Run, layout);

            await SendMessageWithStartAsync(message);
        }

        public async void Stop() => await SendAsync();

        public async void Quit() => await SendAsync();

        public async void LoadAtStartup(bool state = true) => await SendAsync();

        public async void CommandLine(IList<string> args) => await SendAsync();

        public async void Running() => await SendAsync();

        private readonly SemaphoreSlim _startingSemaphore = new SemaphoreSlim(1, 1);


        private async Task<bool> StartDaemonAsync()
        {
            //await StopDaemon();

            await _startingSemaphore.WaitAsync();
            try
            {
                if (_client != null)
                {
                    if (_client.IsConnected) return true;
                    _client = null;
                }
                /*
                var args = Debugger.IsAttached?"debug":"";
                var module = Process.GetCurrentProcess().MainModule;

                var filename = module?.FileName;
                if (filename == null) return false;

                filename = filename.Replace(".Control.exe", ".Daemon.exe").Replace(".vshost", "");

                _daemonProcess = new Process
                {
                    StartInfo = new ProcessStartInfo(filename)
                    {
                        RedirectStandardOutput = true,
                        RedirectStandardInput = true,
                        Arguments = args,
                        WindowStyle = ProcessWindowStyle.Hidden,
                        CreateNoWindow = true,
                    }
                };
                if (_daemonProcess == null) return false;
                if (!_daemonProcess.Responding) return false;
                if (_daemonProcess.HasExited) return false;
                */

                _client = new PipeClient<DaemonMessage>("LBM");
                await _client.ConnectAsync();


                _client.MessageReceived += (sender, args) => OnStateChanged(args.Message.State);

                _client.Disconnected += (sender, args) => {};

                return true;
            }
            finally
            {
                _startingSemaphore.Release();
            }
        }
        private async Task StopDaemon()
        {
            await SendMessageWithStartAsync(new DaemonMessage(LittleBigMouseCommand.Stop,null));
        }

        private void _daemonProcess_Exited(object sender, EventArgs e)
        {
            throw new NotImplementedException();
        }




        private Task SendAsync([CallerMemberName]string name = null)
        {
            if(name==null) throw new ArgumentNullException(nameof(name));
            if (name.EndsWith("Async")) name = name[..^5];
            return Enum.TryParse<LittleBigMouseCommand>(name, out var command) ? SendMessageWithStartAsync(new DaemonMessage(command,null)) : Task.CompletedTask;
        }

        private async Task SendMessageWithStartAsync(DaemonMessage message)
        {
            var retry = (_client != null) || await StartDaemonAsync();

            while (retry)
            {
                try
                {
                    if (!_client.IsConnected) continue;

                    await SendMessageAsync(message);
                    return;
                }
                catch (TimeoutException)
                {
                    retry = await StartDaemonAsync();
                }

            }
        }

        private async Task SendMessageAsync(DaemonMessage message)
        {
            var serializer = new DataContractSerializer(typeof(DaemonMessage),new DataContractSerializerSettings() { PreserveObjectReferences = true});

            var ms = new MemoryStream();

            var xsn = new XmlSerializerNamespaces();
            xsn.Add(string.Empty, string.Empty);


            serializer.WriteObject(ms, message);

            using var sr = new StreamReader(ms);

            ms.Position = 0;

            var xml = await sr.ReadToEndAsync();   

            await _client.WriteAsync(message); //.StandardInput.WriteAsync(xml);


        }


    }

}