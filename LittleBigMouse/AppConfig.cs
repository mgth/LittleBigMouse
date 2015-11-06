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
using LbmScreenConfig;

namespace LittleBigMouse
{

    public class AppConfig : Application, ISingleInstanceApp, IDisposable
    {

        private void _formConfig_RegistryChanged(object sender, EventArgs e)
        {
            //TODO : reload
        }

        private FormConfig _formConfig = null;
        private void ShowConfig()
        {
            if (_formConfig==null)
            {
                _formConfig = new FormConfig(new ScreenConfig());
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

        public void Stop()
        {

        }


        public bool SignalExternalCommandLineArgs(IList<string> args)
        {
            if (args[0]=="--exit")
            {
                Shutdown();
            }
            return true;
        }

        private bool _disposed = false;
        protected virtual void Dispose(bool disposing)
        {
            if (_disposed) return;

            if (disposing)
            {
                Stop();
                if (_formConfig != null)
                {
                    _formConfig.Dispose();
                    _formConfig = null;
                }

            }
            _disposed = true;
        }
        public void Dispose()
        {
            Dispose(true);
        }
    }

}
