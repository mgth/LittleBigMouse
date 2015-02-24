/*
  MouseControl - Mouse Managment in multi DPI monitors environment
  Copyright (c) 2015 Mathieu GRENET.  All right reserved.

  This file is part of MouseControl.

    ArduixPL is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    ArduixPL is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/

using Microsoft.Shell;
using Microsoft.Win32.TaskScheduler;
using System;
using System.Diagnostics;
using System.Security.Principal;
using System.Windows;

namespace LittleBigMouse
{
    /// <summary>
    /// Interaction logic for App.xaml
    /// </summary>
    public partial class App : Application
    {
        private const string Unique = "MgthLittleBigMouseApp";

        [STAThread]
        public static void Main(string[] args)
        {
            int nbarg = 0;
            bool silent = false;

            foreach (string s in args)
            {
                if (s == "--schedule")
                {
                    Schedule();
                    nbarg++;
                    break;
                }
                if (s == "--unschedule")
                {
                    Unschedule();
                    nbarg++;
                    break;
                }
                if (s == "--silent")
                {
                    silent=true;
                    break;
                }
            }

            if (nbarg == 0 && SingleInstance<AppConfig>.InitializeAsFirstInstance(Unique))
            {
                

                using (AppConfig application = new AppConfig())
                {
                    application.ShutdownMode = ShutdownMode.OnExplicitShutdown;

                    application.Start(silent);

                    application.Run();
                }

                SingleInstance<AppConfig>.Cleanup();
            }
        }




        public static bool IsAdministrator
        {
            get
            {
                WindowsIdentity identity = WindowsIdentity.GetCurrent();
                WindowsPrincipal principal = new WindowsPrincipal(identity);
                return principal.IsInRole(WindowsBuiltInRole.Administrator);
            }
        }

        public static String ServiceName
        {
            get { return System.Windows.Forms.Application.ProductName; }
        }

        private static void AdminCommand(String cmd)
        {
            // Restart program and run as admin
            var exeName = Process.GetCurrentProcess().MainModule.FileName;
            ProcessStartInfo startInfo = new ProcessStartInfo(exeName);
            startInfo.Verb = "runas";
            startInfo.Arguments = cmd;
            Process.Start(startInfo);
            return;
        }

        public static bool Scheduled
        {
            get
            {
                using (TaskService ts = new TaskService())
                {
                    Microsoft.Win32.TaskScheduler.Task t = ts.FindTask(ServiceName);
                    if (t == null) return false;
                    if (t.Enabled) return true;
                }
                return false;
            }
        }

        public static void Schedule()
        {
            if (IsAdministrator)
            {
                using (TaskService ts = new TaskService())
                {
                    ts.RootFolder.DeleteTask(ServiceName,false);

                    TaskDefinition td = ts.NewTask();
                    td.RegistrationInfo.Description = "Multi-dpi aware monitors mouse crossover";
                    td.Triggers.Add(new LogonTrigger());
                    td.Actions.Add(
                        new ExecAction(System.Windows.Forms.Application.ExecutablePath.ToString(),"--silent")
                        );

                    td.Principal.RunLevel = TaskRunLevel.Highest;
                    td.Settings.DisallowStartIfOnBatteries = false;
                    td.Settings.DisallowStartOnRemoteAppSession = true;
                    td.Settings.StopIfGoingOnBatteries = false;
                    td.Settings.ExecutionTimeLimit = TimeSpan.Zero;

                    ts.RootFolder.RegisterTaskDefinition(ServiceName, td);
                }
            }
            else AdminCommand("--schedule");
        }
        public static void Unschedule()
        {
            if (IsAdministrator)
            {
                using (TaskService ts = new TaskService())
                {
                    ts.RootFolder.DeleteTask(ServiceName, false);
                }
            }
            else AdminCommand("--unschedule");
        }
    }
}
