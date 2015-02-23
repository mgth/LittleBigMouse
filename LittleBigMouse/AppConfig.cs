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
        private ScreenConfig _config;

        private double _mouseSpeed = Mouse.MouseSpeed;
        private Notify _notify = new Notify();
        private void LoadConfig()
        {
            if (_config != null) _config.Disable();
            _config = ScreenConfig.Load(Registry.CurrentUser);
            _config.Enable();
        }
        private void _formConfig_RegistryChanged(object sender, EventArgs e)
        {
            LoadConfig();
        }
        private void _notify_Click(object sender, EventArgs e)
        {
            ShowConfig();
        }

        private FormConfig _formConfig = null;
        private void ShowConfig()
        {
            if (_formConfig==null)
            {
                _formConfig = new FormConfig(_config);
                _formConfig.RegistryChanged += _formConfig_RegistryChanged;
                _formConfig.Closed += _formConfig_Closed;
                _formConfig.Show();
            }
            else
            {
                _formConfig.Activate();
            }
        }

        private void _formConfig_Closed(object sender, EventArgs e)
        {
            _formConfig = null;
        }

        public void Start(bool silent)
        {
            LoadConfig();
            if (!silent) { ShowConfig(); }
            _notify.Click += _notify_Click;
            Exit += AppConfig_Exit;
        }
        public void Stop()
        {
            if (_config!=null)
            {
                _config.Disable();

                if (_config.AdjustSpeed)
                    Mouse.MouseSpeed = 10.0;

                if (_config.AdjustPointer)
                    Mouse.setCursorAero(1);
            }
            if (_notify!=null)
                _notify.Dispose();
        }

        private void AppConfig_Exit(object sender, ExitEventArgs e)
        {
            Stop();
        }

        public bool SignalExternalCommandLineArgs(IList<string> args)
        {
            if (args[0]=="--exit")
            {
                Shutdown();
            }
            return true;
        }
    }

}
