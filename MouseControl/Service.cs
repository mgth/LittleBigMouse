using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Data;
using System.Diagnostics;
using System.Linq;
using System.ServiceProcess;
using System.Text;
using System.Threading.Tasks;

namespace MouseControl
{
    partial class Service : ServiceBase
    {
        public Service()
        {
            InitializeComponent();
        }

        protected override void OnStart(string[] args)
        {
            var application = new App();
            application.InitializeComponent();

            LoadConfig();
            _notify.Click += _notify_Click;

            application.Run();

        }

        protected override void OnStop()
        {
            Mouse.MouseSpeed = 10.0;
            Mouse.setCursorAero(1);

            _notify.Dispose();
            // TODO: ajoutez ici le code pour effectuer les destructions nécessaires à l'arrêt de votre service.
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
