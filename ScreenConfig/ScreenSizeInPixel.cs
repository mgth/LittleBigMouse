using System;
using System.Collections.Generic;
using System.Linq;
using System.Runtime.CompilerServices;
using System.Text;
using System.Threading.Tasks;
using Erp.Notify;
using Microsoft.Win32;

namespace LbmScreenConfig
{
    public class ScreenSizeInPixels : ScreenSize
    {
        public ScreenSizeInPixels(Screen screen)
        {
            Screen = screen;
            this.Subscribe();
        }



//        [TriggedOn("Screen.Monitor.DisplayOrientation")]
        [TriggedOn("Screen.Monitor.MonitorArea")]
        public override double Width
        {
            get => this.Get(() => Screen.Monitor.MonitorArea.Width);
            //get => this.Get(() => Screen.Monitor.DisplayOrientation % 2 == 0 ? Screen.Monitor.MonitorArea.Width : Screen.Monitor.MonitorArea.Height);
            set => throw new NotImplementedException();
        }

//        [TriggedOn("Screen.Monitor.DisplayOrientation")]
        [TriggedOn("Screen.Monitor.MonitorArea")]
        public override double Height
        {
            get => this.Get(() => Screen.Monitor.MonitorArea.Height);
            //get => this.Get(() => Screen.Monitor.DisplayOrientation % 2 == 0 ? Screen.Monitor.MonitorArea.Height : Screen.Monitor.MonitorArea.Width);
            set => throw new NotImplementedException();
        }

        [TriggedOn("Screen.Monitor.MonitorArea")]
        public override double X
        {
            get => this.Get(() => Screen.Monitor.MonitorArea.X);
            set => throw new NotImplementedException();
        }

        [TriggedOn("Screen.Monitor.MonitorArea")]
        public override double Y
        {
            get => this.Get(() => Screen.Monitor.MonitorArea.Y);
            set => throw new NotImplementedException();
        }
        public override double TopBorder
        {
            get => this.Get(() => 0);
            set => throw new NotImplementedException();
        }
        public override double BottomBorder
        {
            get => this.Get(() => 0);
            set => throw new NotImplementedException();
        }
        public override double LeftBorder
        {
            get => this.Get(() => 0);
            set => throw new NotImplementedException();
        }
        public override double RightBorder
        {
            get => this.Get(() => 0);
            set => throw new NotImplementedException();
        }
        private double LoadValueMonitor(Func<double> def, [CallerMemberName]string name = null)
        {
            using (RegistryKey key = Screen.OpenMonitorRegKey())
            {
                return key.GetKey(name, def);
            }
        }
        private double LoadValueConfig(Func<double> def, [CallerMemberName]string name = null)
        {
            using (RegistryKey key = Screen.OpenConfigRegKey())
            {
                return key.GetKey(name, def);
            }
        }
    }
}
