using System;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
using System.Runtime.Serialization;
using System.Windows;

using HLab.Notify.PropertyChanged;

using Microsoft.Win32;

using Newtonsoft.Json;

using NativeMethods = HLab.Sys.Windows.API.NativeMethods;

namespace HLab.Sys.Windows.Monitors
{
    using H = H<MonitorDevice>;

    [DataContract]
    public class MonitorDevice : NotifierBase
    {
        public MonitorDevice(string deviceId, IMonitorsService service)
        {
            MonitorsService = service;
            DeviceId = deviceId;

            H.Initialize(this);

            SetEdid();
        }

        [DataMember] public string DeviceId { get; }

        [JsonIgnore] public IMonitorsService MonitorsService { get; }

        public RegistryKey OpenMonitorRegKey(bool create = false)
        {
            using var key = MonitorsService.OpenRootRegKey(create);

            if (key == null) return null;
            return create ? key.CreateSubKey(@"monitors\" + IdMonitor) : key.OpenSubKey(@"monitors\" + IdMonitor);
        }

        public string ConfigPath(bool create = false)
        {
            var path = Path.Combine(MonitorsService.AppDataPath(create), IdMonitor);
            if (create) Directory.CreateDirectory(path);

            return path;
        }

        [DataMember] 
        public string PnpCode => _pnpCode.Get();
        private readonly IProperty<string> _pnpCode = H.Property<string>(c => c
            .Set(s =>
            {
                var id = s.DeviceId.Split('\\');
                return id.Length > 1 ? id[1] : id[0];
            })
            .On(e => e.DeviceId).Update()
        );

        [DataMember] 
        public string IdMonitor => _idMonitor.Get();
        private readonly IProperty<string> _idMonitor = H.Property<string>(c => c
             .Set(s => s.GetMicrosoftId())
             .On(e => e.PnpCode)
             .On(e => e.Edid.SerialNo)
             .On(e => e.Edid.Week)
             .On(e => e.Edid.Year)
             .On(e => e.Edid.Checksum)
             .On(e => e.DeviceId)
             .Update()
         );
        private string GetMicrosoftId()
        {
            if(Edid != null)
                return $"{PnpCode}{Edid.SerialNo}_{Edid.Week:X2}_{Edid.Year:X4}_{Edid.Checksum:X2}";

            return $"NOEDID_{PnpCode}_{DeviceId.Split('\\').Last()}";
        }

        [DataMember] 
        public string IdPhysicalMonitor => _idPhysicalMonitor.Get();
        private readonly IProperty<string> _idPhysicalMonitor = H.Property<string>(c => c
             .Set(s => s.GetPhysicalId())
             .On(e => e.PnpCode)
             .On(e => e.Edid.SerialNo)
             .On(e => e.Edid.Week)
             .On(e => e.Edid.Year)
             //.On(e => e.Edid.Checksum)
             .On(e => e.DeviceId)
             .Update()
         );
        private string GetPhysicalId()
        {
            if(Edid != null)
                return $"{PnpCode}{Edid.SerialNo}_{Edid.Week:X2}_{Edid.Year:X4}"/*_{Edid.Checksum:X2}*/;

            return $"NOEDID_{PnpCode}_{DeviceId.Split('\\').Last()}";
        }

        [DataMember] 
        public string DeviceKey
        {
            get => _deviceKey.Get();
            internal set => _deviceKey.Set(value);
        }
        private readonly IProperty<string> _deviceKey = H.Property<string>();

        [DataMember]
        public string DeviceString
        {
            get => _deviceString.Get();
            internal set => _deviceString.Set(value ?? "");
        }
        private readonly IProperty<string> _deviceString = H.Property<string>();

        [DataMember]
        public IObservableFilter<DisplayDevice> Devices { get; }
         = H.Filter<DisplayDevice>(c => c
             .On(e => e.DeviceId).On(e => e.MonitorsService.Devices.Item().DeviceId)
             .Update()
             .AddFilter((e, a) => a.DeviceId == e.DeviceId)
             .Link(e => e.MonitorsService.Devices)
         );


        [DataMember]
        public DisplayDevice AttachedDevice
        {
            get => _attachedDevice.Get();
            set => _attachedDevice.Set(value);
        }
        private readonly IProperty<DisplayDevice> _attachedDevice = H.Property<DisplayDevice>();

        [JsonProperty]
        public DisplayDevice AttachedDisplay
        {
            get => _attachedDisplay.Get();
            set => _attachedDisplay.Set(value);
        }
        private readonly IProperty<DisplayDevice> _attachedDisplay = H.Property<DisplayDevice>();

        internal void SetMonitorInfoEx(NativeMethods.MONITORINFOEX mi)
        {
            Primary = mi.Flags == 1;
            MonitorArea = mi.Monitor;
            WorkArea = mi.WorkArea;
        }

        [DataMember]
        public Rect MonitorArea // MONITORINFOEX 
        {
            get => _monitorArea.Get();
            private set => _monitorArea.Set(value);
        }
        private readonly IProperty<Rect> _monitorArea = H.Property<Rect>();

        [DataMember]
        public Rect WorkArea // MONITORINFOEX
        {
            get => _workArea.Get();
            private set => _workArea.Set(value);
        }
        private readonly IProperty<Rect> _workArea = H.Property<Rect>();

        [DataMember]
        public string HKeyName // ZwQueryKey / NativeMethods.KEY_INFORMATION_CLASS.KeyNameInformation
        {
            get => _hKeyName.Get();
            set => _hKeyName.Set(value);
        }
        private readonly IProperty<string> _hKeyName = H.Property<string>();

        [DataMember]
        public bool AttachedToDesktop // EnumDisplayDevices
        {
            get => _attachedToDesktop.Get();
            set => _attachedToDesktop.Set(value);
        }
        private readonly IProperty<bool> _attachedToDesktop = H.Property<bool>();


        [DataMember]
        public bool Primary // MONITORINFOEX
        {
            get => _primary.Get();
            internal set
            {
                // Must remove old primary screen before setting this one
                if (value)
                {
                    foreach (var monitor in MonitorsService.Monitors.Where(m => !m.Equals(this)))
                    {
                        monitor.Primary = false;
                    }
                }
                _primary.Set(value);
            }
        }
        private readonly IProperty<bool> _primary = H.Property<bool>();

        [DataMember]
        public Edid Edid => _edid.Get();
        private readonly IProperty<Edid> _edid = H.Property<Edid>();

        private void SetEdid()
        {
            IntPtr devInfo = NativeMethods.SetupDiGetClassDevsEx(
                ref NativeMethods.GUID_CLASS_MONITOR, //class GUID
                null, //enumerator
                IntPtr.Zero, //HWND
                NativeMethods.DIGCF_PRESENT | NativeMethods.DIGCF_PROFILE, // Primary //DIGCF_ALLCLASSES|
                IntPtr.Zero, // device info, create a new one.
                null, // machine name, local machine
                IntPtr.Zero
            ); // reserved

            try
            {
                if (devInfo == IntPtr.Zero)
                {
                    _edid.Set(null);
                    return;
                }

                var devInfoData = new NativeMethods.SP_DEVINFO_DATA(true);

                uint i = 0;

                do
                {
                    int result = 0;

                    if (NativeMethods.SetupDiEnumDeviceInfo(devInfo, i, ref devInfoData))
                    {

                        var hEdidRegKey = NativeMethods.SetupDiOpenDevRegKey(devInfo, ref devInfoData,
                            NativeMethods.DICS_FLAG_GLOBAL, 0, NativeMethods.DIREG_DEV, NativeMethods.KEY_READ);

                        try
                        {
                            if (hEdidRegKey != IntPtr.Zero && (hEdidRegKey.ToInt32() != -1))
                            {
                                using var key = GetKeyFromPath(GetHKeyName(hEdidRegKey), 1);

                                var value = key.GetValue("HardwareID");
                                if (value is string[] s && s.Length > 0)
                                {
                                    var id = s[0] + "\\" +
                                             key.GetValue("Driver");

                                    if (id == DeviceId)
                                    {
                                        HKeyName = GetHKeyName(hEdidRegKey);
                                        using var keyEdid = GetKeyFromPath(HKeyName);

                                        var edid = (byte[])keyEdid.GetValue("EDID");
                                        if (edid != null) _edid.Set(new Edid(edid));
                                        return;
                                    }
                                }
                            }
                        }
                        finally
                        {
                            result = NativeMethods.RegCloseKey(hEdidRegKey);
                            if (result > 0)
                                throw new Exception(NativeMethods.GetLastErrorString());
                        }
                    }


                    i++;
                } while (NativeMethods.ERROR_NO_MORE_ITEMS != NativeMethods.GetLastError());
            }
            finally
            {
                NativeMethods.SetupDiDestroyDeviceInfoList(devInfo);
            }

            _edid.Set(null);
            return;

        }

        public IntPtr HMonitor { get; private set; }
        public void UpdateDpi(IntPtr hMonitor)
        {
            HMonitor = hMonitor;

            {
                GetDpiForMonitor(hMonitor, DpiType.Effective, out var x, out var y);
                _effectiveDpi.Set(new Vector(x, y));
            }
            {
                GetDpiForMonitor(hMonitor, DpiType.Angular, out var x, out var y);
                _angularDpi.Set(new Vector(x, y));
            }
            {
                GetDpiForMonitor(hMonitor, DpiType.Raw, out var x, out var y);
                _rawDpi.Set(new Vector(x, y));
            }
            {
                var factor = 100;
                NativeMethods.GetScaleFactorForMonitor(hMonitor, ref factor);
                _scaleFactor.Set((double)factor / 100.0);
            }

            _capabilitiesString.Set(NativeMethods.DDCCIGetCapabilitiesString(hMonitor));
        }



        //private readonly IProperty<IntPtr> _hPhysical = H.Property<IntPtr>(nameof(HPhysical));
        //public IntPtr HPhysical => _hPhysical.Get();

        private enum DpiType
        {
            Effective = 0,
            Angular = 1,
            Raw = 2,
        }        //https://msdn.microsoft.com/en-us/library/windows/desktop/dn280510.aspx

        [DataMember] public Vector EffectiveDpi => _effectiveDpi.Get(); //GetDpiForMonitor
        private readonly IProperty<Vector> _effectiveDpi = H.Property<Vector>();
        [DataMember] public Vector AngularDpi => _angularDpi.Get(); //GetDpiForMonitor
        private readonly IProperty<Vector> _angularDpi = H.Property<Vector>();
        [DataMember] public Vector RawDpi => _rawDpi.Get(); //GetDpiForMonitor
        private readonly IProperty<Vector> _rawDpi = H.Property<Vector>();

        //https://msdn.microsoft.com/fr-fr/library/windows/desktop/dn302060.aspx
        [DataMember] public double ScaleFactor => _scaleFactor.Get(); //GetScaleFactorForMonitor
        private readonly IProperty<double> _scaleFactor = H.Property<double>();

        [DataMember] public string CapabilitiesString => _capabilitiesString.Get(); //DDCCIGetCapabilitiesString
        private readonly IProperty<string> _capabilitiesString = H.Property<string>();





        [DllImport("Shcore.dll")]
        private static extern IntPtr GetDpiForMonitor([In] IntPtr hmonitor, [In] DpiType dpiType, [Out] out uint dpiX, [Out] out uint dpiY);


        [DataMember]
        public int MonitorNo
        {
            get => _monitorNo.Get();
            set => _monitorNo.Set(value);
        }
        private readonly IProperty<int> _monitorNo = H.Property<int>(c => c);

        public string WallpaperPath
        {
            get => _wallpaperPath.Get();
            set => _wallpaperPath.Set(value);
        }
        private readonly IProperty<string> _wallpaperPath = H.Property<string>(c => c);

        //------------------------------------------------------------------------
        public override bool Equals(object obj) => obj is MonitorDevice other ? DeviceId == other.DeviceId : base.Equals(obj);


        public override int GetHashCode()
        {
            return ("DisplayMonitor" + DeviceId).GetHashCode();
        }
        public static RegistryKey GetKeyFromPath(string path, int parent = 0)
        {
            var keys = path.Split('\\');

            RegistryKey key;

            switch (keys[2])
            {
                case "USER": key = Registry.CurrentUser; break;
                case "CONFIG": key = Registry.CurrentConfig; break;
                default: key = Registry.LocalMachine; break;
            }

            for (var i = 3; i < (keys.Length - parent); i++)
            {
                if (key == null) return key;
                key = key.OpenSubKey(keys[i]);
            }

            return key;
        }
        public static string GetHKeyName(IntPtr hKey)
        {
            var result = string.Empty;
            var pKNI = IntPtr.Zero;

            var needed = 0;
            var status = NativeMethods.ZwQueryKey(hKey, NativeMethods.KEY_INFORMATION_CLASS.KeyNameInformation, IntPtr.Zero, 0, out needed);
            if (status != 0xC0000023) return result;

            pKNI = Marshal.AllocHGlobal(cb: sizeof(uint) + needed + 4 /*paranoia*/);
            status = NativeMethods.ZwQueryKey(hKey, NativeMethods.KEY_INFORMATION_CLASS.KeyNameInformation, pKNI, needed, out needed);
            if (status == 0)    // STATUS_SUCCESS
            {
                var bytes = new char[2 + needed + 2];
                Marshal.Copy(pKNI, bytes, 0, needed);
                // startIndex == 2  skips the NameLength field of the structure (2 chars == 4 bytes)
                // needed/2         reduces value from bytes to chars
                //  needed/2 - 2    reduces length to not include the NameLength
                result = new string(bytes, 2, (needed / 2) - 2);
            }
            Marshal.FreeHGlobal(pKNI);
            return result;
        }
        private void OpenRegKey(string keystring)
        {
            using (RegistryKey key = Registry.CurrentUser.OpenSubKey(@"Software\Microsoft\Windows\CurrentVersion\Applets\Regedit\Lastkey", true))
            {
                //key;
            }
        }

        public void DisplayValues(Action<string, string, RoutedEventHandler, bool> addValue)
        {
            addValue("Registry", HKeyName, (sender, args) => { OpenRegKey(HKeyName); }, false);
            addValue("Microsoft Id", IdMonitor, null, false);


            // EnumDisplaySettings
            addValue("", "EnumDisplaySettings", null, true);
            addValue("DisplayOrientation", AttachedDisplay?.CurrentMode?.DisplayOrientation.ToString(), null, false);
            addValue("Position", AttachedDisplay?.CurrentMode?.Position.ToString(), null, false);
            addValue("Pels", AttachedDisplay?.CurrentMode?.Pels.ToString(), null, false);
            addValue("BitsPerPixel", AttachedDisplay?.CurrentMode?.BitsPerPixel.ToString(), null, false);
            addValue("DisplayFrequency", AttachedDisplay?.CurrentMode?.DisplayFrequency.ToString(), null, false);
            addValue("DisplayFlags", AttachedDisplay?.CurrentMode?.DisplayFlags.ToString(), null, false);
            addValue("DisplayFixedOutput", AttachedDisplay?.CurrentMode?.DisplayFixedOutput.ToString(), null, false);

            // GetDeviceCaps
            addValue("", "GetDeviceCaps", null, true);
            addValue("Size", AttachedDisplay?.DeviceCaps.Size.ToString(), null, false);
            addValue("Res", AttachedDisplay?.DeviceCaps.Resolution.ToString(), null, false);
            addValue("LogPixels", AttachedDisplay?.DeviceCaps.LogPixels.ToString(), null, false);
            addValue("BitsPixel", AttachedDisplay?.DeviceCaps.BitsPixel.ToString(), null, false);
            //AddValue("Color Planes", Monitor.Adapter.DeviceCaps.Planes.ToString());
            addValue("Aspect", AttachedDisplay?.DeviceCaps.Aspect.ToString(), null, false);
            //AddValue("BltAlignment", Monitor.Adapter.DeviceCaps.BltAlignment.ToString());

            //GetDpiForMonitor
            addValue("", "GetDpiForMonitor", null, true);
            addValue("EffectiveDpi", EffectiveDpi.ToString(), null, false);
            addValue("AngularDpi", AngularDpi.ToString(), null, false);
            addValue("RawDpi", RawDpi.ToString(), null, false);

            // GetMonitorInfo
            addValue("", "GetMonitorInfo", null, true);
            addValue("Primary", Primary.ToString(), null, false);
            addValue("MonitorArea", MonitorArea.ToString(), null, false);
            addValue("WorkArea", WorkArea.ToString(), null, false);


            // EDID
            addValue("", "EDID", null, true);
            addValue("ManufacturerCode", Edid?.ManufacturerCode, null, false);
            addValue("ProductCode", Edid?.ProductCode, null, false);
            addValue("Serial", Edid?.Serial, null, false);
            addValue("Model", Edid?.Model, null, false);
            addValue("SerialNo", Edid?.SerialNo, null, false);
            addValue("SizeInMm", Edid?.PhysicalSize.ToString(), null, false);

            // GetScaleFactorForMonitor
            addValue("", "GetScaleFactorForMonitor", null, true);
            addValue("ScaleFactor", ScaleFactor.ToString(), null, false);

            // EnumDisplayDevices
            addValue("", "EnumDisplayDevices", null, true);
            addValue("DeviceId", AttachedDisplay?.DeviceId, null, false);
            addValue("DeviceKey", AttachedDisplay?.DeviceKey, null, false);
            addValue("DeviceString", AttachedDisplay?.DeviceString, null, false);
            addValue("DeviceName", AttachedDisplay?.DeviceName, null, false);
            addValue("StateFlags", AttachedDisplay?.State.ToString(), null, false);

            addValue("", "EnumDisplayDevices", null, true);
            addValue("DeviceId", DeviceId, null, false);
            addValue("DeviceKey", DeviceKey, null, false);
            addValue("DeviceString", DeviceString, null, false);
            addValue("DeviceName", AttachedDevice?.DeviceName, null, false);
            addValue("StateFlags", AttachedDevice?.State.ToString(), null, false);

        }
        public override string ToString() => DeviceString;

    }
}
