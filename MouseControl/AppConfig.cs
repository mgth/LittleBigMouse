using Microsoft.Win32;
using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Windows;

namespace MouseControl
{

    public class AppConfig : Application
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
            ScreenConfig scr = ScreenConfig.Load(Registry.CurrentUser);

            scr.RegistryChanged += Scr_RegistryChanged;

            FormConfig cfg = new FormConfig(scr);
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
    }

}
