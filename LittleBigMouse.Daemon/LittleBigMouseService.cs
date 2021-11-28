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
using System.Reflection;
using System.Threading.Tasks;
using HLab.Sys.Windows.Monitors;
using LittleBigMouse.ScreenConfig;

namespace LittleBigMouse.Daemon
{


    #if USESERVICE
    class LittleBigMouseService : ILittleBigMouseService
    {
        private MouseEngine _engine;
        private readonly IMonitorsService _monitorService;

        public LittleBigMouseService(IMonitorsService monitorService)
        {
            _monitorService = monitorService;
        }

        protected void OnStart(string[] args)
        {
            base.OnStart(args);
            _engine = new MouseEngine(_monitorService);
            _engine.Start();
        }
        protected void OnStop()
        {
            base.OnStop();
            _engine.Stop();
        }

        public void Init()
        {
            throw new System.NotImplementedException("Service : Init");
        }

        public async Task LoadConfig()
        {
            _engine.LoadConfig();
        }

        public async Task Quit()
        {
            base.Stop();
        }

        public async Task<bool> Start()
        {
            _engine.Start();
            return true;
        }
        public new bool Stop()
        {
            base.Stop();
            return true;
        }

        public async Task LoadAtStartup(bool state = true)
        {
            if (state)
                InstallService();
            else
                UninstallService();
        }

        public async Task CommandLine(IList<string> args)
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

        public async Task<bool> Running()
        {
            return true; // TODO : _engine.Hook.Enabled;
        }

        public async Task Update()
        {
            throw new System.NotImplementedException();
        }

        public void OnStateChange()
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
#endif
}
