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
using System;
using System.Collections.Generic;
using System.Configuration;
using System.Data;
using System.Linq;
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

        [STAThread]
        public static void Main()
        {

            if (SingleInstance<App>.InitializeAsFirstInstance(Unique))
            {
                var application = new App();
                application.InitializeComponent();

                LoadConfig();
                _notify.Click += _notify_Click;

                application.Run();

                // Allow single instance code to perform cleanup operations
                SingleInstance<App>.Cleanup();

                Mouse.MouseSpeed = 10.0;
                Mouse.setCursorAero(1);

                _notify.Dispose();
            }
        }

        public bool SignalExternalCommandLineArgs(IList<string> args)
        {
            return true;
        }

        private static ScreenConfig _config;

        private static double _mouseSpeed = Mouse.MouseSpeed;
        private static Notify _notify = new Notify();

        private static void LoadConfig()
        {
            if (_config != null) _config.Disable();
            _config = ScreenConfig.Load();
            _config.Enable();
        }
        private static void _notify_Click(object sender, EventArgs e)
        {
            ScreenConfig scr = ScreenConfig.Load();

            scr.RegistryChanged += Scr_RegistryChanged;

            FormConfig cfg = new FormConfig(scr);
            cfg.Show();
        }
        private static void Scr_RegistryChanged(object sender, EventArgs e)
        {
            LoadConfig();
        }

    }
}
