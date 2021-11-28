/*
  LittleBigMouse.Daemon
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

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
using System.Threading;
using System.Windows;
using HLab.Notify;
using HLab.Remote;

namespace LittleBigMouse.Daemon
{
    //[ServiceBehavior(InstanceContextMode = InstanceContextMode.Single)]
    internal class Program
    {
        private const string UNIQUE = "LittleBigMouse_Daemon";
        [STAThread]
        public static void Main(string[] args)
        {
            var mutex = new Mutex(true, UNIQUE + Environment.UserName, out var firstInstance);

            if (!firstInstance)
            {
                var client = new RemoteClient("lbm.daemon");
                foreach (var arg in args)
                {
                    client.SendMessageAsync(arg);
                }
                mutex.Close();
                return;
            }

            if (Environment.UserInteractive)
            {
                var daemon = new LittleBigMouseDaemon{ ShutdownMode = ShutdownMode.OnExplicitShutdown };
                daemon.Run();
            }
            else
            {
                #if USESERVICE
                var evt = new EventHandlerServiceWpf();
                var servicesToRun = new ServiceBase[]
                {
                    new LittleBigMouseService(new MonitorsService())
                };
                ServiceBase.Run(servicesToRun);
                #endif
            }

            mutex.Close();
            Environment.Exit(0);
        }
    }
}
