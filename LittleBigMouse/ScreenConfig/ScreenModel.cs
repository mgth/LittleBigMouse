using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using HLab.Notify;
using HLab.Windows.Monitors;
using Microsoft.Win32;
using Newtonsoft.Json;

namespace LittleBigMouse.ScreenConfigs
{
    public class ScreenModel : NotifierObject
    {
        public ScreenConfig Config { get; }
        public string PnpCode { get; }

        public ScreenModel(string pnpCode, ScreenConfig config)
        {
            Config = config;
            PnpCode = pnpCode;
        }

        [JsonProperty]
        public string PnpDeviceName
        {
            get => this.Get<string>();
            private set
            {
                if (this.Set(value)) Saved = false;
            }
        }


        [TriggedOn(nameof(Physical), "TopBorder")]
        [TriggedOn(nameof(Physical), "RightBorder")]
        [TriggedOn(nameof(Physical), "BottomBorder")]
        [TriggedOn(nameof(Physical), "LeftBorder")]
        [TriggedOn(nameof(Physical), "Height")]
        [TriggedOn(nameof(Physical), "Width")]
        public void SetSaved()
        {
            Saved = false;
        }

        [JsonProperty]
        public ScreenSizeInMm Physical => this.Get(() => new ScreenSizeInMm(this));

        public void Save(RegistryKey baseKey)
        {
            if (Saved) return;

            using (RegistryKey key = OpenMonitorRegKey(true))
            {
                if (key != null)
                {
                    key.SetKey("TopBorder", Physical.TopBorder.ToString(CultureInfo.InvariantCulture));
                    key.SetKey("RightBorder", Physical.RightBorder.ToString(CultureInfo.InvariantCulture));
                    key.SetKey("BottomBorder", Physical.BottomBorder.ToString(CultureInfo.InvariantCulture));
                    key.SetKey("LeftBorder", Physical.LeftBorder.ToString(CultureInfo.InvariantCulture));

                    key.SetKey("Height", Physical.Height.ToString(CultureInfo.InvariantCulture));
                    key.SetKey("Width", Physical.Width.ToString(CultureInfo.InvariantCulture));

                    key.SetKey("PnpName", PnpDeviceName);
                    //key.SetKey("DeviceId", Monitor.DeviceId);
                }
            }
            Saved = true;
        }

        public ScreenModel Load(Monitor monitor)
        {
            var display = monitor.AttachedDisplay;
            var old = Physical.FixedAspectRatio;
            Physical.FixedAspectRatio = false;

            InitSize(monitor);

            using (RegistryKey key = OpenMonitorRegKey(false))
            {

                if (key != null)
                {
                    Physical.TopBorder = double.Parse(key.GetValue("TopBorder", Physical.TopBorder).ToString(), CultureInfo.InvariantCulture);
                    Physical.RightBorder = double.Parse(key.GetValue("RightBorder", Physical.RightBorder).ToString(), CultureInfo.InvariantCulture);
                    Physical.BottomBorder = double.Parse(key.GetValue("BottomBorder", Physical.BottomBorder).ToString(), CultureInfo.InvariantCulture);
                    Physical.LeftBorder = double.Parse(key.GetValue("LeftBorder", Physical.LeftBorder).ToString(), CultureInfo.InvariantCulture);

                    Physical.Height = double.Parse(key.GetValue("Height", Physical.Height).ToString(), CultureInfo.InvariantCulture);
                    Physical.Width = double.Parse(key.GetValue("Whidth", Physical.Width).ToString(), CultureInfo.InvariantCulture);

                    PnpDeviceName = key.GetValue("PnpName", "").ToString();

                    //key.SetKey("DeviceId", Monitor.DeviceId);

                }
                    if (string.IsNullOrEmpty(PnpDeviceName))
                    {
                        var name = Html.CleanupPnpName(monitor.DeviceString);
                        if(name.ToLower()=="generic pnp monitor") name = Html.GetPnpName(PnpCode);

                        PnpDeviceName = name;                       
                    }

                Physical.FixedAspectRatio = old;
            }
            Saved = true;

            return this;
        }

        public void InitSize(Monitor monitor)
        {
            var display = monitor.AttachedDisplay;
            var old = Physical.FixedAspectRatio;
            Physical.FixedAspectRatio = false;

            if (display != null)
                switch ((display.CurrentMode.DisplayOrientation) % 2)
                {
                    case 0:
                        Physical.Width = display.DeviceCaps.Size.Width;
                        Physical.Height = display.DeviceCaps.Size.Height;
                        break;
                    case 1:
                        Physical.Width = display.DeviceCaps.Size.Height;
                        Physical.Height = display.DeviceCaps.Size.Width;
                        break;
                }

            Physical.FixedAspectRatio = old;
        }

        public static RegistryKey OpenMonitorRegKey(string id, bool create = false)
        {
            using (RegistryKey key = ScreenConfig.OpenRootRegKey(create))
            {
                if (key == null) return null;
                return create ? key.CreateSubKey(@"monitors\" + id) : key.OpenSubKey(@"monitors\" + id);
            }
        }

        public RegistryKey OpenMonitorRegKey(bool create = false)
        {
            return OpenMonitorRegKey(PnpCode, create);
        }

        public bool Saved
        {
            get => this.Get<bool>();
            set => this.Set(value);
        }

    }
}
