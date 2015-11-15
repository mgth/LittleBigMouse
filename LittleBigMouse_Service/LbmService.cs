using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Data;
using System.Diagnostics;
using System.Linq;
using System.ServiceProcess;
using System.Text;
using System.Threading;
using System.Threading.Tasks;

namespace LittleBigMouse_Service
{
    public partial class LbmService : ServiceBase
    {
        public LbmService()
        {
            InitializeComponent();
        }

        private MouseEngine _engine;
        protected override void OnStart(string[] args)
        {
            base.OnStart(args);
            _engine = new MouseEngine();
            _engine?.Start();
        }

        protected override void OnStop()
        {
            base.OnStop();
            _engine?.Stop();
        }
    }
}
