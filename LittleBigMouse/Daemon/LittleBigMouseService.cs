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
using System.Collections.Generic;
using System.Configuration.Install;
using System.Reflection;
using System.ServiceProcess;
using HLab.Base;
using HLab.Windows.Monitors;
using LittleBigMouse.ScreenConfigs;

namespace LittleBigMouse_Daemon
{
    class LittleBigMouseService : ServiceBase, ILittleBigMouseService
    {
        private MouseEngine _engine;
        private readonly IMonitorsService _monitorService;

        public LittleBigMouseService(IMonitorsService monitorService)
        {
            _monitorService = monitorService;
        }

        protected override void OnStart(string[] args)
        {
            base.OnStart(args);
            _engine = new MouseEngine(_monitorService);
            _engine.Start();
        }
        protected override void OnStop()
        {
            base.OnStop();
            _engine.Stop();
        }

        public void Init()
        {
            throw new System.NotImplementedException("Service : Init");
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

        public bool Running()
        {
            return true; // TODO : _engine.Hook.Enabled;
        }

        public void Update()
        {
            throw new System.NotImplementedException();
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
