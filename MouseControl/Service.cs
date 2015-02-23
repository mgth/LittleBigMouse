using Microsoft.Win32;
using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Data;
using System.Diagnostics;
using System.Linq;
using System.ServiceProcess;
using System.Text;
using System.Threading.Tasks;

namespace LittleBigMouse
{
    partial class Service : ServiceBase
    {
        public Service()
        {
            InitializeComponent();
        }

        protected override void OnStart(string[] args)
        {
            App.log("onstart");
            LoadConfig();
        }

        protected override void OnStop()
        {
            App.log("onstop");
            Mouse.MouseSpeed = 10.0;
            Mouse.setCursorAero(1);
        }

        private static ScreenConfig _config;

        private static double _mouseSpeed = Mouse.MouseSpeed;

        private static void LoadConfig()
        {
            if (_config != null) _config.Disable();
            _config = ScreenConfig.Load(Registry.LocalMachine);
            _config.Enable();
        }
    }
}
