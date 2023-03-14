using System;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Reactive.Linq;
using System.Runtime.InteropServices;
using System.Runtime.Serialization;
using Avalonia;
using DynamicData;
using HLab.Sys.Windows.API;
using Microsoft.Win32;
using Newtonsoft.Json;
using ReactiveUI;

using static HLab.Sys.Windows.API.ErrHandlingApi;
using static HLab.Sys.Windows.API.SetupApi;
using static HLab.Sys.Windows.API.ShellScalingApi;
using static HLab.Sys.Windows.API.WinBase;

namespace HLab.Sys.Windows.Monitors
{
    [DataContract]
    public class MonitorDevice : ReactiveObject
    {
        public MonitorDevice(string deviceId, IMonitorsService service)
        {
            MonitorsService = service;
            DeviceId = deviceId;
            PnpCode = GetPnpCodeFromId(deviceId);
            Edid = GetEdid(deviceId);
            IdMonitor = GetMicrosoftId(deviceId, Edid);

            Devices = MonitorsService
                .Devices
                .Connect()
                .Filter(e => e.DeviceId == deviceId)
                .ObserveOn(RxApp.MainThreadScheduler)
                .AsObservableCache();
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
        public string PnpCode { get; }

        static string GetPnpCodeFromId(string deviceId)
        {
            var id = deviceId.Split('\\');
            return id.Length > 1 ? id[1] : id[0];
        }

        [DataMember] public string IdMonitor { get; }

        static string GetMicrosoftId(string deviceId, Edid edid)
        {
            var pnpCode = GetPnpCodeFromId(deviceId);

            return edid is null 
                ? GetPhysicalId(deviceId, null)
                : $"{GetPhysicalId(deviceId, edid)}_{edid.Checksum:X2}";
        }

        [DataMember] public string IdPhysicalMonitor { get; }

        static string GetPhysicalId(string deviceId, Edid edid)
        {
            var pnpCode = GetPnpCodeFromId(deviceId);
            return edid == null 
                ? $"NOEDID_{pnpCode}_{deviceId.Split('\\').Last()}" 
                : $"{pnpCode}{edid.SerialNo}_{edid.Week:X2}_{edid.Year:X4}" /*_{Edid.Checksum:X2}*/;
        }

        [DataMember] 
        public string DeviceKey
        {
            get => _deviceKey;
            internal set => this.RaiseAndSetIfChanged(ref _deviceKey, value);
        }
        string _deviceKey;

        [DataMember]
        public string DeviceString
        {
            get => _deviceString;
            internal set => this.RaiseAndSetIfChanged(ref _deviceString, value ?? "");
        }
        string _deviceString;

        [DataMember]
        public IObservableCache<DisplayDevice,string> Devices { get; }


        [DataMember]
        public DisplayDevice AttachedDevice
        {
            get => _attachedDevice;
            set => this.RaiseAndSetIfChanged(ref _attachedDevice, value);
        }
        DisplayDevice _attachedDevice;

        [JsonProperty]
        public DisplayDevice AttachedDisplay
        {
            get => _attachedDisplay;
            set => this.RaiseAndSetIfChanged(ref _attachedDisplay, value);
        }
        DisplayDevice _attachedDisplay;

        internal void SetMonitorInfo(WinUser.MonitorInfoEx mi)
        {
            Primary = mi.Flags == 1;
            MonitorArea = mi.Monitor.ToRect();
            WorkArea = mi.WorkArea.ToRect();
        }
        
        internal void SetMonitorInfo(WinUser.MonitorInfo mi)
        {
            Primary = mi.Flags == 1;
            MonitorArea = mi.Monitor.ToRect();
            WorkArea = mi.WorkArea.ToRect();
        }

        [DataMember]
        public Rect MonitorArea // MONITORINFOEX 
        {
            get => _monitorArea;
            private set => this.RaiseAndSetIfChanged(ref _monitorArea, value);
        }
        Rect _monitorArea;

        [DataMember]
        public Rect WorkArea // MONITORINFOEX
        {
            get => _workArea;
            private set => this.RaiseAndSetIfChanged(ref _workArea, value);
        }

        Rect _workArea;

        [DataMember]
        public bool AttachedToDesktop // EnumDisplayDevices
        {
            get => _attachedToDesktop;
            set => this.RaiseAndSetIfChanged(ref _attachedToDesktop, value);
        }
        bool _attachedToDesktop;


        [DataMember]
        public bool Primary // MONITORINFOEX
        {
            get => _primary;
            internal set
            {
                // Must remove old primary screen before setting this one
                if (value)
                {
                    foreach (var monitor in MonitorsService.Monitors.Items.Where(m => !m.Equals(this)))
                    {
                        monitor.Primary = false;
                    }
                }
                this.RaiseAndSetIfChanged(ref _primary, value);
            }
        }
        bool _primary;

        [DataMember]
        public Edid Edid { get; }

        static Edid GetEdid(string deviceId)
        {
            var devInfo = SetupDiGetClassDevsEx(
                ref GUID_CLASS_MONITOR, //class GUID
                null, //enumerator
                0, //HWND
                DIGCF_PRESENT | DIGCF_PROFILE, // Primary //DIGCF_ALLCLASSES|
                0, // device info, create a new one.
                null, // machine name, local machine
                0
            ); // reserved

            try
            {
                if (devInfo == 0)
                {
                    return null;
                }

                var devInfoData = new SP_DEVINFO_DATA();

                uint i = 0;

                do
                {
                    if (SetupDiEnumDeviceInfo(devInfo, i, ref devInfoData))
                    {

                        var hEdidRegKey = SetupDiOpenDevRegKey(devInfo, ref devInfoData,
                            DICS_FLAG_GLOBAL, 0, DIREG_DEV, KEY_READ);

                        try
                        {
                            if (hEdidRegKey != 0 && ((int)hEdidRegKey != -1))
                            {
                                using var key = GetKeyFromPath(GetHKeyName(hEdidRegKey), 1);
                                var value = key?.GetValue("HardwareID");
                                if (value is string[] s && s.Length > 0)
                                {
                                    var id = s[0] + "\\" +
                                             key.GetValue("Driver");

                                    if (id == deviceId)
                                    {
                                        var hKeyName = GetHKeyName(hEdidRegKey);
                                        using var keyEdid = GetKeyFromPath(hKeyName);

                                        var edid = (byte[])keyEdid.GetValue("EDID");
                                        return edid != null ? new Edid(hKeyName,edid) : null;
                                    }
                                }
                            }
                        }
                        finally
                        {
                            var result = WinReg.RegCloseKey(hEdidRegKey);
                            if (result > 0)
                                throw new Exception(GetLastErrorString());
                        }
                    }


                    i++;
                } while (ErrorNoMoreItems != GetLastError());
            }
            finally
            {
                SetupDiDestroyDeviceInfoList(devInfo);
            }

            return null;
        }

        //public nint HMonitor { get; private set; }
        public void UpdateDpi(nint hMonitor)
        {
            //HMonitor = hMonitor;

            {
                var hResult = GetDpiForMonitor(hMonitor, DpiType.Effective, out var x, out var y);
                if (hResult != 0)
                {
                    var errorCode = Marshal.GetLastWin32Error();
                    Debug.WriteLine("GetDpiForMonitor failed with error code: {0}", errorCode);
                }
                
                EffectiveDpi = new Vector(x, y);
            }
            {
                if (GetDpiForMonitor(hMonitor, DpiType.Angular, out var x, out var y) != 0)
                {
                    var errorCode = Marshal.GetLastWin32Error();
                    Debug.WriteLine("GetDpiForMonitor failed with error code: {0}", errorCode);
                }
                AngularDpi = new Vector(x, y);
            }
            {
                if (GetDpiForMonitor(hMonitor, DpiType.Raw, out var x, out var y) != 0)
                {
                    var errorCode = Marshal.GetLastWin32Error();
                    Debug.WriteLine("GetDpiForMonitor failed with error code: {0}", errorCode);
                }
                RawDpi = new Vector(x, y);
            }
            {
                var factor = 100;
                if (GetScaleFactorForMonitor(hMonitor, ref factor) != 0)
                {
                    var errorCode = Marshal.GetLastWin32Error();
                    Debug.WriteLine("GetScaleFactorForMonitor failed with error code: {0}", errorCode);
                }
                ScaleFactor = (double)factor / 100.0;
            }

            CapabilitiesString = Gdi32.DDCCIGetCapabilitiesString(hMonitor);
        }



        //private readonly IProperty<IntPtr> _hPhysical = H.Property<IntPtr>(nameof(HPhysical));
        //public IntPtr HPhysical => _hPhysical.Get();


        [DataMember] public Vector EffectiveDpi
        {
            get => _effectiveDpi; //GetDpiForMonitor
            set => this.RaiseAndSetIfChanged(ref _effectiveDpi, value);
        }

        Vector _effectiveDpi;
        [DataMember] public Vector AngularDpi
        {
            get => _angularDpi; //GetDpiForMonitor
            set => _angularDpi = value;
        }

        Vector _angularDpi;
        [DataMember] public Vector RawDpi
        {
            get => _rawDpi; //GetDpiForMonitor
            set => this.RaiseAndSetIfChanged(ref _rawDpi, value);
        }

        Vector _rawDpi;

        //https://msdn.microsoft.com/fr-fr/library/windows/desktop/dn302060.aspx
        [DataMember] public double ScaleFactor
        {
            get => _scaleFactor; //GetScaleFactorForMonitor
            set => this.RaiseAndSetIfChanged(ref _scaleFactor, value);
        }

        double _scaleFactor;

        [DataMember] public string CapabilitiesString
        {
            get => _capabilitiesString; //DDCCIGetCapabilitiesString
            set => this.RaiseAndSetIfChanged(ref _capabilitiesString, value);
        }

        string _capabilitiesString;







        [DataMember]
        public int MonitorNo
        {
            get => _monitorNo;
            set => this.RaiseAndSetIfChanged(ref _monitorNo, value);
        }

        int _monitorNo;

        public string WallpaperPath
        {
            get => _wallpaperPath;
            set => this.RaiseAndSetIfChanged(ref _wallpaperPath, value);
        }
        string _wallpaperPath;

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
        public static string GetHKeyName(nint hKey)
        {
            var result = string.Empty;

            var status = Wdm.ZwQueryKey(hKey, Wdm.KeyInformationClass.KeyNameInformation, 0, 0, out var needed);
            if (status != 0xC0000023) return result;

            var pKni = Marshal.AllocHGlobal(cb: sizeof(uint) + needed + 4 /*paranoia*/);
            status = Wdm.ZwQueryKey(hKey, Wdm.KeyInformationClass.KeyNameInformation, pKni, needed, out needed);
            if (status == 0)    // STATUS_SUCCESS
            {
                var bytes = new char[2 + needed + 2];
                Marshal.Copy(pKni, bytes, 0, needed);
                // startIndex == 2  skips the NameLength field of the structure (2 chars == 4 bytes)
                // needed/2         reduces value from bytes to chars
                //  needed/2 - 2    reduces length to not include the NameLength
                result = new string(bytes, 2, (needed / 2) - 2);
            }
            Marshal.FreeHGlobal(pKni);
            return result;
        }

        void OpenRegKey(string keystring)
        {
            using (RegistryKey key = Registry.CurrentUser.OpenSubKey(@"Software\Microsoft\Windows\CurrentVersion\Applets\Regedit\Lastkey", true))
            {
                //key;
            }
        }

        public void DisplayValues(Action<string, string, Action, bool> addValue)
        {
            addValue("Registry", Edid?.HKeyName, () => { OpenRegKey(Edid?.HKeyName); }, false);
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
