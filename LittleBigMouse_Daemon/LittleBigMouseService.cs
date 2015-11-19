using System.Collections.Generic;
using System.Configuration.Install;
using System.Reflection;
using System.ServiceProcess;
using LbmScreenConfig;

namespace LittleBigMouse_Daemon
{
    class LittleBigMouseService : ServiceBase, ILittleBigMouseService
    {
        private MouseEngine _engine;
        protected override void OnStart(string[] args)
        {
            base.OnStart(args);
            _engine = new MouseEngine();
            _engine.Start();
        }
        protected override void OnStop()
        {
            base.OnStop();
            _engine.Stop();
        }

        public void LoadConfig()
        {
            _engine.LoadConfig();
        }

        public void Quit()
        {
            base.Stop();
        }

        public void Start()
        {
            _engine.Start();
        }

        public void LoadAtStartup(bool state = true)
        {
            if (state)
                InstallService();
            else
                UninstallService();
        }

        public void CommandLine(IList<string> args)
        {
            foreach (string s in args)
            {
                switch (s)
                {
                    case "--exit":
                        Stop();
                        this.Quit();
                        //Shutdown();
                        break;
                    case "--load":
                        LoadConfig();
                        break;
                    case "--start":
                        Start();
                        break;
                    case "--stop":
                        Stop();
                        break;
                    case "--schedule":
                        LoadAtStartup(true);
                        break;
                    case "--unschedule":
                        LoadAtStartup(false);
                        break;
                }
            }
        }

        private static void InstallService()
        {
            try
            {
                ManagedInstallerClass.InstallHelper(new string[]
            { Assembly.GetExecutingAssembly().Location });
            }
            catch { }
        }

        private static void UninstallService()
        {
            ManagedInstallerClass.InstallHelper(new string[]
            { "/u", Assembly.GetExecutingAssembly().Location });
        }

    }
}
