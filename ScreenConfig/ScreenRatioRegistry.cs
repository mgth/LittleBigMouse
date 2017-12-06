using System;
using System.Runtime.CompilerServices;
using Erp.Notify;
using Microsoft.Win32;

namespace LbmScreenConfig
{

    public class ScreenRatioRegistry : ScreenRatio
    {
        private readonly string _prefix;

        public ScreenRatioRegistry(Screen screen,[CallerMemberName] string prefix = null)
        {
            Screen = screen;
            _prefix = prefix;
            this.Subscribe();
        }

        public Screen Screen
        {
            get => this.Get<Screen>();
            private set => this.Set(value);
        }
        public override double X
        {
            get => this.Get(() => LoadValue(() => 1.0));
            set { if (this.Set(value)) Screen.Config.Saved = false; }
        }
        public override double Y
        {
            get => this.Get(() => LoadValue(() => 1.0));
            set { if (this.Set(value)) Screen.Config.Saved = false; }
        }

        double LoadValue(Func<double> def, [CallerMemberName]string name = null)
        {
            using (RegistryKey key = Screen.OpenConfigRegKey())
            {
                return key.GetKey(_prefix + "." + name, def);
            }
        }
    }
}