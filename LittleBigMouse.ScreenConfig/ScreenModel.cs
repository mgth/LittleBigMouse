using System.Globalization;
using System.Runtime.Serialization;
using HLab.Notify.Annotations;
using HLab.Notify.PropertyChanged;
using HLab.Sys.Windows.Monitors;
using LittleBigMouse.ScreenConfig.Dimensions;
using Microsoft.Win32;
using Newtonsoft.Json;

namespace LittleBigMouse.ScreenConfig
{
    using H = H<ScreenModel>;

    [DataContract]
    public class ScreenModel : NotifierBase
    {
        [JsonIgnore]
        public ScreenConfig Config { get; }
        public string PnpCode { get; }

        public ScreenModel(string pnpCode, ScreenConfig config)
        {
            Config = config;
            PnpCode = pnpCode;
            H.Initialize(this);
        }

        [DataMember]
        public string PnpDeviceName
        {
            get => _pnpDeviceName.Get();
            private set
            {
                if (_pnpDeviceName.Set(value)) Saved = false;
            }
        }
        private readonly IProperty<string> _pnpDeviceName = H.Property<string>();


        private ITrigger _ = H.Trigger(c => c
            .On(e => e.Physical.TopBorder)
            .On(e => e.Physical.RightBorder)
            .On(e => e.Physical.BottomBorder)
            .On(e => e.Physical.LeftBorder)
            .On(e => e.Physical.Height)
            .On(e => e.Physical.Width)
            .Do(e => e.Saved = false)
        );

        [DataMember] public ScreenSizeInMm Physical => _physical.Get();
        private readonly IProperty<ScreenSizeInMm> _physical = H.Property<ScreenSizeInMm>(c => c
            .Set(e => new ScreenSizeInMm(e))
        );

        public void Save(RegistryKey baseKey)
        {
            if (Saved) return;

            using (var key = OpenMonitorRegKey(true))
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

            if (display?.CurrentMode != null)
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
            get => _saved.Get();
            set => _saved.Set(value);
        }
        private readonly IProperty<bool> _saved = H.Property<bool>();

    }
}
