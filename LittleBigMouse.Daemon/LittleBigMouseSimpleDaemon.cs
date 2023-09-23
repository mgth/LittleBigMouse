/*
  LittleBigMouse.Daemon
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

  This file is part of LittleBigMouse.Daemon.

    LittleBigMouse.Daemon is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    LittleBigMouse.Daemon is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/

using System;
using System.IO;
using System.Runtime.Serialization;
using System.Threading.Tasks;
using System.Windows;
using H.Pipes;
using H.Pipes.Args;
using LittleBigMouse.Zoning;

using Newtonsoft.Json;


namespace LittleBigMouse.Daemon
{
    internal class LittleBigMouseSimpleDaemon : Application
    {
        private MouseEngine _engine;
        private PipeServer<DaemonMessage> _server;

        public LittleBigMouseSimpleDaemon()
        {
            ShutdownMode = ShutdownMode.OnExplicitShutdown;
        }

        protected override async void OnStartup(StartupEventArgs e)
        {
            base.OnStartup(e);

            _server = new PipeServer<DaemonMessage>("LBM");

            _server.ClientConnected += async (o, args) => await OnClientConnectedAsync(args);
            _server.ClientDisconnected += (o, args) => OnClientDisconnected(args);
            _server.MessageReceived += (sender, args) => OnMessageReceived(args.Message);
            _server.ExceptionOccurred += (o, args) => OnExceptionOccurred(args.Exception);

            await _server.StartAsync();

            var w = new Window();
            w.Show();
        }

        private void OnClientDisconnected(ConnectionEventArgs<DaemonMessage> args)
        {
        }

        private void OnExceptionOccurred(Exception argsException)
        {
            throw new NotImplementedException();
        }

        private async Task OnClientConnectedAsync(ConnectionEventArgs<DaemonMessage> args)
        {
            Console.WriteLine($"Client {args.Connection.ServerName} is now connected!");

            await args.Connection.WriteAsync(new DaemonMessage(LittleBigMouseCommand.Run,null) );
        }

        private void OnMessageReceived(DaemonMessage message)
        {
            switch (message.Command)
            {
                case LittleBigMouseCommand.Run:
                    _engine?.Stop();
                    _engine = new MouseEngine(message.Payload);
                    _engine.Start();

                    break;
                case LittleBigMouseCommand.Stop:
                    _engine?.Stop();
                    break;
                case LittleBigMouseCommand.Quit:
                    Application.Current.Shutdown();
                    break;
                default:
                    throw new ArgumentOutOfRangeException();
            }
        }
    }
}
