using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO.Pipes;
using System.Linq.Expressions;
using System.Threading;
using System.Threading.Tasks;
using System.Windows.Documents;
using HLab.DependencyInjection.Annotations;
using HLab.Remote;

namespace LittleBigMouse.ScreenConfig
{

     [Export(typeof(ILittleBigMouseClientService)),Singleton]
    public class LittleBigMouseClientService : RemoteClient, ILittleBigMouseClientService
    {
        public event Action<string> StateChanged;

        protected void OnStateChanged(string state)
        {
            StateChanged?.Invoke(state);
        }

        public LittleBigMouseClientService():base("lbm.daemon")
        {
        }



        public async void LoadConfig() => SendAsync();
        public async void Start()
        {
            var ack = await SendAsync();
            if(ack=="ACK") OnStateChanged("running");
        }

        public async void Stop()
        {
            var ack = await SendAsync();
            if(ack=="ACK") OnStateChanged("stopped");

        }

        public async void Update() => SendAsync();
        public async void Quit() => SendAsync();

        public async void LoadAtStartup(bool state = true) => SendAsync();

        public async void CommandLine(IList<string> args) => SendAsync();

        public async void Running() => SendAsync();

        private Process _daemonProcess = null;
        private readonly SemaphoreSlim _startingSemaphore = new SemaphoreSlim(1,1);
        protected override async Task<bool> StartServerAsync()
        {
            await _startingSemaphore.WaitAsync();
            try
            {
                if (_daemonProcess != null)
                {
                    if (!_daemonProcess.HasExited && _daemonProcess.Responding) return true;
                    _daemonProcess = null;
                }

                var args = "";
                var module = Process.GetCurrentProcess().MainModule;

                var filename = module?.FileName;
                if (filename == null) return false;

                filename = filename.Replace(".Control.exe", ".Daemon.exe").Replace(".vshost", "");
                try
                {
                    _daemonProcess = Process.Start(filename, args);
                }
                catch (Exception ex)
                {

                }

                if (_daemonProcess == null) return false;
                if (!_daemonProcess.Responding) return false;
                if (_daemonProcess.HasExited) return false;
                return true;
            }
            finally
            {
                _startingSemaphore.Release();
            }


        }

    }

}