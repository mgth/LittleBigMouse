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
using Microsoft.Win32;
using Microsoft.Win32.TaskScheduler;
using System;
using System.Collections.Generic;
using System.Configuration;
using System.Data;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Security.Principal;
using System.ServiceProcess;
using System.Threading.Tasks;
using System.Windows;

namespace LittleBigMouse
{
    /// <summary>
    /// Interaction logic for App.xaml
    /// </summary>
    public partial class App : Application, ISingleInstanceApp
    {
        private const string Unique = "MgthMouseControlApp";

        public static void log(String s)
        {
            using (StreamWriter w = File.AppendText("F:\\docs\\_docs_\\_projets\\MouseControl\\MouseControl\\bin\\Release\\log.txt"))
            {
                w.WriteLine(s);
            }
        }

        [STAThread]
        public static void Main(string[] args)
        {
            int nbarg = 0;

            log("main");

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
            }

            if (nbarg == 0 && SingleInstance<App>.InitializeAsFirstInstance(Unique))
            {
                var application = new AppConfig();

                application.ShutdownMode = ShutdownMode.OnExplicitShutdown;
                //application.InitializeComponent();

                application.Start();

                application.Run();

                // Allow single instance code to perform cleanup operations
                SingleInstance<App>.Cleanup();
            }
        }

        public bool SignalExternalCommandLineArgs(IList<string> args)
        {
            return true;
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
                    td.RegistrationInfo.Description = "Multi-dpi monitors mouse control";
                    td.Triggers.Add(new LogonTrigger());
                    td.Actions.Add(
                        new ExecAction(System.Windows.Forms.Application.ExecutablePath.ToString())
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
