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

namespace MouseControl
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

            foreach(string s in args)
            {
                log(s);
                if (s=="--service")
                {
                    ServiceBase.Run( new ServiceBase[] { new Service() });
                    nbarg++;
                    break;
                }

                if (s=="--install")
                {
                    InstallService();
                    nbarg++;
                    break;
                }

                if (s=="--start")
                {
                    StartService();
                    nbarg++;
                    break;
                }

                if (s == "--stop")
                {
                    StopService();
                    nbarg++;
                    break;
                }

                if (s == "--restart")
                {
                    RestartService();
                    nbarg++;
                    break;
                }
            }

            if (nbarg==0 && SingleInstance<App>.InitializeAsFirstInstance(Unique))
            {
                var application = new AppConfig();
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
                //                Application.Current.Shutdown();
                return;
        }

        public static void InstallService()
        {
            if (IsAdministrator)
            {
                ConfigService();

                if (ServiceInstaller.ServiceIsInstalled(ServiceName))
                    ServiceInstaller.Uninstall(ServiceName);

                    ServiceInstaller.InstallAndStart(
                        ServiceName,
                        ServiceName,
                        System.Windows.Forms.Application.ExecutablePath.ToString() + " --service"
                        );
            }
            else AdminCommand("--install");
        }

        public static void ConfigService()
        {
            try
            {
                log("a");
                ScreenConfig cfg = ScreenConfig.Load(Registry.CurrentUser);
                log("b");
                cfg.Save(Registry.LocalMachine);
                log("c");
            }
            catch (ApplicationException e)
            {
                log(e.ToString());
            }
        }

        public static void StartService()
        {
            if (IsAdministrator)
            {
                ConfigService();

                ServiceController service = ServiceController.GetServices().FirstOrDefault(i => i.ServiceName.Contains(ServiceName));

                if (service.Status != ServiceControllerStatus.Running)
                {
                    service.Start();//  /// Cannot open SERVICENAME service on computer '.'. 
                    service.WaitForStatus(ServiceControllerStatus.Running);
                }
            }
            else AdminCommand("--start");
        }

        public static void StopService()
        {
            if (IsAdministrator)
            {
                ServiceController service = ServiceController.GetServices().FirstOrDefault(i => i.ServiceName.Contains(ServiceName));

                if (service.Status == ServiceControllerStatus.Running)
                {
                    service.Stop();//  /// Cannot open SERVICENAME service on computer '.'. 
                    service.WaitForStatus(ServiceControllerStatus.Stopped);
                }
            }
            else AdminCommand("--stop");
        }

        public static void RestartService()
        {
            if (IsAdministrator)
            {
                StopService();
                StartService();
            }
            else AdminCommand("--restart");
        }

    }
}
