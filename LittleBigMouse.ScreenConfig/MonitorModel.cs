using System.Globalization;
using System.Runtime.Serialization;

using HLab.Notify.Annotations;
using HLab.Notify.PropertyChanged;
using HLab.Sys.Windows.Monitors;

using LittleBigMouse.DisplayLayout.Dimensions;

using Microsoft.Win32;

using Newtonsoft.Json;

namespace LittleBigMouse.DisplayLayout
{
    using H = H<MonitorModel>;

    [DataContract]
    public class MonitorModel : NotifierBase
    {
        [JsonIgnore]
        public Layout Config { get; }
        public string PnpCode { get; }

        public MonitorModel(string pnpCode, Layout config)
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
            .On(e => e.PhysicalSize.TopBorder)
            .On(e => e.PhysicalSize.RightBorder)
            .On(e => e.PhysicalSize.BottomBorder)
            .On(e => e.PhysicalSize.LeftBorder)
            .On(e => e.PhysicalSize.Height)
            .On(e => e.PhysicalSize.Width)
            .Do(e => e.Saved = false)
        );

        [DataMember] public DisplaySizeInMm PhysicalSize => _physicalSize.Get();
        private readonly IProperty<DisplaySizeInMm> _physicalSize = H.Property<DisplaySizeInMm>(c => c
            .Set(e => new DisplaySizeInMm(e))
        );

        public void Save(RegistryKey baseKey)
        {
            if (Saved) return;

            using (var key = OpenMonitorRegKey(true))
            {
                if (key != null)
                {
                    key.SetKey("TopBorder", PhysicalSize.TopBorder.ToString(CultureInfo.InvariantCulture));
                    key.SetKey("RightBorder", PhysicalSize.RightBorder.ToString(CultureInfo.InvariantCulture));
                    key.SetKey("BottomBorder", PhysicalSize.BottomBorder.ToString(CultureInfo.InvariantCulture));
                    key.SetKey("LeftBorder", PhysicalSize.LeftBorder.ToString(CultureInfo.InvariantCulture));

                    key.SetKey("Height", PhysicalSize.Height.ToString(CultureInfo.InvariantCulture));
                    key.SetKey("Width", PhysicalSize.Width.ToString(CultureInfo.InvariantCulture));

                    key.SetKey("PnpName", PnpDeviceName);
                    //key.SetKey("DeviceId", Monitor.DeviceId);
                }
            }
            Saved = true;
        }

        public MonitorModel Load(MonitorDevice monitor)
        {
            var display = monitor.AttachedDisplay;
            var old = PhysicalSize.FixedAspectRatio;
            PhysicalSize.FixedAspectRatio = false;

            InitSize(monitor);

            using (RegistryKey key = OpenMonitorRegKey(false))
            {

                if (key != null)
                {
                    PhysicalSize.TopBorder = double.Parse(key.GetValue("TopBorder", PhysicalSize.TopBorder).ToString(), CultureInfo.InvariantCulture);
                    PhysicalSize.RightBorder = double.Parse(key.GetValue("RightBorder", PhysicalSize.RightBorder).ToString(), CultureInfo.InvariantCulture);
                    PhysicalSize.BottomBorder = double.Parse(key.GetValue("BottomBorder", PhysicalSize.BottomBorder).ToString(), CultureInfo.InvariantCulture);
                    PhysicalSize.LeftBorder = double.Parse(key.GetValue("LeftBorder", PhysicalSize.LeftBorder).ToString(), CultureInfo.InvariantCulture);

                    PhysicalSize.Height = double.Parse(key.GetValue("Height", PhysicalSize.Height).ToString(), CultureInfo.InvariantCulture);
                    PhysicalSize.Width = double.Parse(key.GetValue("Whidth", PhysicalSize.Width).ToString(), CultureInfo.InvariantCulture);

                    PnpDeviceName = key.GetValue("PnpName", "").ToString();

                    //key.SetKey("DeviceId", Monitor.DeviceId);

                }

                if (string.IsNullOrEmpty(PnpDeviceName))
                {
                    var name = Html.CleanupPnpName(monitor.DeviceString);
                    //if (name.ToLower() == "generic pnp monitor") name = Html.GetPnpName(PnpCode);

                    PnpDeviceName = name;
                }

                PhysicalSize.FixedAspectRatio = old;
            }
            Saved = true;

            return this;
        }

        public void InitSize(MonitorDevice monitor)
        {
            var old = PhysicalSize.FixedAspectRatio;
            PhysicalSize.FixedAspectRatio = false;

            var display = monitor.AttachedDisplay;

            if (display?.CurrentMode != null)
                switch (display.CurrentMode.DisplayOrientation % 2)
                {
                    case 0:

                        PhysicalSize.Width = display.DeviceCaps.Size.Width;
                        PhysicalSize.Height = display.DeviceCaps.Size.Height;
                        break;
                    case 1:
                        PhysicalSize.Width = display.DeviceCaps.Size.Height;
                        PhysicalSize.Height = display.DeviceCaps.Size.Width;
                        break;
                }
            else if (monitor.Edid != null)
            {
                PhysicalSize.Width = monitor.Edid.PhysicalSize.Width;
                PhysicalSize.Height = monitor.Edid.PhysicalSize.Height;
            }

            PhysicalSize.FixedAspectRatio = old;
        }

        public static RegistryKey OpenMonitorRegKey(string id, bool create = false)
        {
            using (RegistryKey key = Layout.OpenRootRegKey(create))
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
