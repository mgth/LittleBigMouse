using System;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Security.Principal;
using Microsoft.Win32.TaskScheduler;

namespace LittleBigMouse_StartUp
{
    class Program
    {
        static void Main(string[] args)
        {
            if (!IsAdministrator)
            {
                AdminCommand(string.Join(" ",args));
                return;
            }

            foreach (string s in args)
            {
                switch (s.ToLower())
                {
                    case "--schedule":
                        Schedule();
                        break;
                    case "--unschedule":
                        Unschedule();
                        break;
                    case "--start":
                    case "--startdaemon":
                        Start("Daemon");
                        break;
                    case "--stop":
                    case "--stopdaemon":
                        Stop("Daemon");
                        break;
                    case "--startcontrol":
                        Start("Control");
                        break;
                    case "--stopcontrol":
                        Stop("Control");
                        break;
                }
            }
        }
        public static bool IsAdministrator
        {
            get
            {
                WindowsIdentity identity = WindowsIdentity.GetCurrent();
                if (identity == null) return false;
                WindowsPrincipal principal = new WindowsPrincipal(identity);
                return principal.IsInRole(WindowsBuiltInRole.Administrator);
            }
        }
        private static void AdminCommand(String cmd)
        {
            // Restart program and run as admin
//            var exeName = Process.GetCurrentProcess().MainModule.FileName;
            string exeName = System.Reflection.Assembly.GetExecutingAssembly().Location;
            ProcessStartInfo startInfo = new ProcessStartInfo(exeName);
            startInfo.Verb = "runas";
            startInfo.Arguments = cmd;
            Process.Start(startInfo);
            return;
        }

        private const string ServiceName = "LittleBigMouse";

        public static void Schedule()
        {
                using (TaskService ts = new TaskService())
                {
                    ts.RootFolder.DeleteTask(ServiceName, false);

                    TaskDefinition td = ts.NewTask();
                    td.RegistrationInfo.Description = "Multi-dpi aware monitors mouse crossover";
                    td.Triggers.Add(new LogonTrigger());

                    var p = Process.GetCurrentProcess();
                    string filename = p.MainModule.FileName.Replace("StartUp","Daemon").Replace(".vshost","");


                    td.Actions.Add(
                        new ExecAction(filename)
                        );

                    td.Principal.RunLevel = TaskRunLevel.Highest;
                    td.Settings.DisallowStartIfOnBatteries = false;
                    td.Settings.DisallowStartOnRemoteAppSession = true;
                    td.Settings.StopIfGoingOnBatteries = false;
                    td.Settings.ExecutionTimeLimit = TimeSpan.Zero;

                    ts.RootFolder.RegisterTaskDefinition(ServiceName, td);
                }
        }
        public static void Unschedule()
        {
                using (TaskService ts = new TaskService())
                {
                    ts.RootFolder.DeleteTask(ServiceName, false);
                }

        }

        [DllImport("USER32.DLL")]
        public static extern bool SetForegroundWindow(IntPtr hWnd);
        public static void Start(string module, bool kill = true)
        {
            bool done = false;

            try
            {
                foreach (Process proc in Process.GetProcessesByName("LittleBigMouse_" + module))
                {
                    if (kill || done) proc.Kill();
                    else
                    {
                        IntPtr hWnd = proc.MainWindowHandle;
                        SetForegroundWindow(hWnd);
                        done = true;
                    }
                }
            }
            catch (Exception ex)
            {

            }

            if (done) return;

            string filename = System.Reflection.Assembly.GetExecutingAssembly().Location.Replace("StartUp", module);
                Process.Start(filename);
        }

        public static void Stop(string module)
        {
            try
            {
                foreach (Process proc in Process.GetProcessesByName("LittleBigMouse_" + module))
                {
                    proc.Kill();
                }
            }
            catch (Exception ex)
            {

            }

        }


    }
}

