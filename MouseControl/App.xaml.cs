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
