using System;
using System.Runtime.CompilerServices;
using Hlab.Notify;
using Microsoft.Win32;

namespace LbmScreenConfig
{
    /// <summary>
    /// Actual real monitor size 
    /// </summary>
    public class ScreenSizeInMm : ScreenSize
    {
        public ScreenSizeInMm(Screen screen)
        {
            Screen = screen;
            this.Subscribe();
        }


        [TriggedOn("Screen", "Orientation")]
        [TriggedOn("Screen", "Monitor","Adapter", "DeviceCaps","Size")]
        public override double Width
        {
            get => this.Get(() => LoadValueMonitor(
                () => Screen.Orientation % 2 == 0
                ? Screen.Monitor.Adapter.DeviceCaps.Size.Width
                : Screen.Monitor.Adapter.DeviceCaps.Size.Height
                , "InMm.Width"));

            set => this.Set(value, (oldValue, newValue) =>
            {
                if (Screen.FixedAspectRatio)
                {
                    var ratio = newValue / oldValue;
                    Screen.FixedAspectRatio = false;
                    Height *= ratio;
                    Screen.FixedAspectRatio = true;
                }

                Screen.Config.Saved = false;
            });
        }

        [TriggedOn("Screen","Orientation")]
        [TriggedOn("Screen", "Monitor", "Adapter", "DeviceCaps", "Size")]
        public override double Height
        {
            get => this.Get(() => LoadValueMonitor(
                ()=>Screen.Orientation % 2 == 0 
                ? Screen.Monitor.Adapter.DeviceCaps.Size.Height 
                : Screen.Monitor.Adapter.DeviceCaps.Size.Width
                ,"InMm.Height"));
            set
            {
                this.Set(value, (oldValue, newValue) =>
                {
                    if (Screen.FixedAspectRatio)
                    {
                        var ratio = newValue / oldValue;
                        Screen.FixedAspectRatio = false;
                        Width *= ratio;
                        Screen.FixedAspectRatio = true;
                    }

                    Screen.Config.Saved = false;
                } );
            }
        }

        public override double X
        {
            get => this.Get(() => LoadValueConfig(() => 0, "InMm.X"));
            set
            {
                if (Screen.Primary)
                {
                    foreach (var screen in Screen.Config.AllBut(Screen))
                    {
                        screen.InMm.X -= value;
                    }
                }
                else if (this.Set(value)) Screen.Config.Saved = false;
            }
        }
        public override double Y
        {
            get => this.Get(() => LoadValueConfig(() => 0,"InMm.Y"));
            set
            {
                if (Screen.Primary)
                {
                    foreach (var screen in Screen.Config.AllBut(Screen))
                    {
                        screen.InMm.Y -= value;
                    }
                }
                else if (this.Set(value)) Screen.Config.Saved = false;
            }
        }
        public override double TopBorder
        {
            get => this.Get(() => LoadValueMonitor(() => 20));
            set { if (this.Set(value)) Screen.Config.Saved = false; }
        }
        public override double BottomBorder
        {
            get => this.Get(() => LoadValueMonitor(() => 20));
            set { if (this.Set(value)) Screen.Config.Saved = false; }
        }
        public override double LeftBorder
        {
            get => this.Get(() => LoadValueMonitor(() => 20));
            set { if (this.Set(value)) Screen.Config.Saved = false; }
        }
        public override double RightBorder
        {
            get => this.Get(() => LoadValueMonitor(()=>20));
            set { if (this.Set(value)) Screen.Config.Saved = false; }
        }
        private double LoadValueMonitor(Func<double> def, [CallerMemberName]string name = null)
        {
            if(Screen==null) throw new PropertyNotReady(0.0); 

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