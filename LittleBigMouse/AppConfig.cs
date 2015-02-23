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
using System.Windows;

namespace LittleBigMouse
{

    public class AppConfig : Application, ISingleInstanceApp
    {
        private static ScreenConfig _config;

        private static double _mouseSpeed = Mouse.MouseSpeed;
        private static Notify _notify = new Notify();
        private static void LoadConfig()
        {
            if (_config != null) _config.Disable();
            _config = ScreenConfig.Load(Registry.CurrentUser);
            _config.Enable();
        }
        private static void Scr_RegistryChanged(object sender, EventArgs e)
        {
            LoadConfig();
        }
        private static void _notify_Click(object sender, EventArgs e)
        {
            FormConfig cfg = new FormConfig(_config);
            cfg.RegistryChanged += Scr_RegistryChanged;
            cfg.Show();
        }
        
        public void Start()
        {
            LoadConfig();
            _notify.Click += _notify_Click;
        }
        public void Stop()
        {
            Mouse.MouseSpeed = 10.0;
            Mouse.setCursorAero(1);

            _notify.Dispose();
        }
        public bool SignalExternalCommandLineArgs(IList<string> args)
        {
            if (args[0]=="--exit")
                Application.Current.Shutdown();
            return true;
        }
    }

}
