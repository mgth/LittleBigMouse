using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Collections.Specialized;
using System.Linq;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading.Tasks;
using System.Windows;
using LbmScreenConfig;
using Microsoft.Win32;
using NotifyChange;
using WinAPI_AdvAPI32;
using WinAPI_Dxva2;
using WinAPI_Gdi32;
using WinAPI_Kernel32;
using WinAPI_Ntdll;
using WinAPI_SetupAPI;
using WinAPI_User32;

namespace LbmScreenConfig
{
    public class DisplayDevice : Notifier
    {
        static DisplayDevice()
        {
            SystemEvents.DisplaySettingsChanged += delegate { UpdateDevices(); };
            UpdateDevices();
        }

        public static ObservableCollection<Adapter> AllAdapters { get; } = new ObservableCollection<Adapter>();
        public static ObservableCollection<Monitor> AllMonitors { get; } = new ObservableCollection<Monitor>();

        public static Adapter FromId(string id)
        {
            return (from monitor in AllMonitors where monitor.DeviceId == id select monitor.Adapter).FirstOrDefault();
        }

        public static void UpdateDevices()
        {
            DISPLAY_DEVICE dev = new DISPLAY_DEVICE(true);
            uint i = 0;

            while (User32.EnumDisplayDevices(null, i++, ref dev, 0))
            {
                Adapter adapter = AllAdapters.FirstOrDefault(d => d.DeviceName == dev.DeviceName);
                if (adapter == null) adapter = new Adapter();
                adapter.DeviceId = dev.DeviceID;
                adapter.DeviceKey = dev.DeviceKey;
                adapter.DeviceName = dev.DeviceName;
                adapter.DeviceString = dev.DeviceString;
                adapter.State = dev.StateFlags;
                
                adapter.UpdateMonitors();
            }


            User32.EnumDisplayMonitors(IntPtr.Zero, IntPtr.Zero,
                delegate (IntPtr hMonitor, IntPtr hdcMonitor, ref RECT lprcMonitor, IntPtr dwData)
                {
                    MONITORINFOEX mi = new MONITORINFOEX(true);
                    bool success = User32.GetMonitorInfo(hMonitor, ref mi);
                    if (success)
                    {
                        var ddDev = AllAdapters.FirstOrDefault(d => d.DeviceName == mi.DeviceName);
                        if (ddDev != null)
                        {
                            if (ddDev.Monitors.Count == 0)
                            {
                                new Monitor(ddDev)
                                {
                                    DeviceName = ddDev.DeviceName + @"\Monitor0",
                                };
                            }

                            foreach (var ddMon in ddDev.Monitors)
                            {
                                ddMon.HMonitor = hMonitor;
                                ddMon.Flags = mi.Flags;
                                ddMon.MonitorArea = mi.Monitor;
                                ddMon.WorkArea = mi.WorkArea;
                            }
                        }
                    }

                    return true;
                }, IntPtr.Zero);

            UpdateEdid();
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
            var status = Ntdll.ZwQueryKey(hKey, KEY_INFORMATION_CLASS.KeyNameInformation, IntPtr.Zero, 0, out needed);
            if (status != 0xC0000023) return result;

            pKNI = Marshal.AllocHGlobal(cb: sizeof(uint) + needed + 4 /*paranoia*/);
            status = Ntdll.ZwQueryKey(hKey, KEY_INFORMATION_CLASS.KeyNameInformation, pKNI, needed, out needed);
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

        public static void UpdateEdid()
        {
            IntPtr devInfo = SetupAPI.SetupDiGetClassDevsEx(
        ref SetupAPI.GUID_CLASS_MONITOR, //class GUID
        null, //enumerator
        IntPtr.Zero, //HWND
        SetupAPI.DIGCF_PRESENT | SetupAPI.DIGCF_PROFILE, // Flags //DIGCF_ALLCLASSES|
        IntPtr.Zero, // device info, create a new one.
        null, // machine name, local machine
        IntPtr.Zero
    );// reserved

            if (devInfo == IntPtr.Zero)
                return;

            SP_DEVINFO_DATA devInfoData = new SP_DEVINFO_DATA(true);

            uint i = 0;
            //            string s = screen.DeviceName.Substring(11);
            //            uint i = 3-uint.Parse(s);

            do
            {
                if (SetupAPI.SetupDiEnumDeviceInfo(devInfo, i, ref devInfoData))
                {

                    IntPtr hEdidRegKey = SetupAPI.SetupDiOpenDevRegKey(devInfo, ref devInfoData,
                        SetupAPI.DICS_FLAG_GLOBAL, 0, SetupAPI.DIREG_DEV, SetupAPI.KEY_READ);

                    if (hEdidRegKey != IntPtr.Zero && (hEdidRegKey.ToInt32() != -1))
                    {
                        using (RegistryKey key = GetKeyFromPath(GetHKeyName(hEdidRegKey), 1))
                        {
                            string id = ((string[])key.GetValue("HardwareID"))[0] + "\\" + key.GetValue("Driver");

                            Monitor mon = AllMonitors.FirstOrDefault(m => m.DeviceId == id);
                            if (mon != null)
                            {
                                mon.HKeyName = GetHKeyName(hEdidRegKey);
                                using (RegistryKey keyEdid = GetKeyFromPath(mon.HKeyName))
                                {
                                    mon.Edid = (byte[])keyEdid.GetValue("EDID");
                                }
                            }
                        }
                        AdvAPI32.RegCloseKey(hEdidRegKey);
                    }
                }
                i++;
            } while (Kernel32.ERROR_NO_MORE_ITEMS != Kernel32.GetLastError());

            SetupAPI.SetupDiDestroyDeviceInfoList(devInfo);
        }

        private string _deviceName = "";
        private string _deviceString = "";
        private DisplayDeviceStateFlags _stateFlags;
        private string _deviceId;
        private string _deviceKey;


        public string DeviceName
        {
            get { return _deviceName; }
            internal set
            {
                if (SetProperty(ref _deviceName, value ?? ""))
                {
                    if (string.IsNullOrWhiteSpace(DeviceString))
                    {
                        string[] s = DeviceName.Split('\\');
                        if (s.Length > 3) DeviceString = s[3];
                    }
                }
            }
        }

        public string DeviceString
        {
            get { return _deviceString; }
            internal set { SetProperty(ref _deviceString, value ?? ""); }
        }

        public DisplayDeviceStateFlags State
        {
            get { return _stateFlags; }
            internal set { SetProperty(ref _stateFlags, value); }
        }

        public string DeviceId
        {
            get { return _deviceId; }
            internal set { SetProperty(ref _deviceId, value); }
        }

        public string DeviceKey
        {
            get { return _deviceKey; }
            internal set { SetProperty(ref _deviceKey, value); }
        }
    }

    public class Adapter : DisplayDevice
    {
        public ObservableCollection<Monitor> Monitors { get; } = new ObservableCollection<Monitor>();
        public Adapter()
        {
            AllAdapters.Add(this);
        }

        ~Adapter()
        {
            AllAdapters.Remove(this);
        }

        public void UpdateMonitors()
        {
            uint i = 0;
            DISPLAY_DEVICE dev = new DISPLAY_DEVICE(true);
            while (User32.EnumDisplayDevices(DeviceName, i++, ref dev, 0))
            {
                Monitor monitor = Monitors.FirstOrDefault(d => d.DeviceId == dev.DeviceID);
                if (monitor == null) monitor = new Monitor(this);
                monitor.DeviceId = dev.DeviceID;
                monitor.DeviceKey = dev.DeviceKey;
                monitor.DeviceName = dev.DeviceName;
                monitor.DeviceString = dev.DeviceString;
                monitor.State = dev.StateFlags;

                monitor.UpdateDevMode();
                monitor.UpdateDeviceCaps();
            }
        }
    }

    public class Monitor : DisplayDevice
    {
        public Adapter Adapter { get; }
        public Monitor(Adapter adapter)
        {
            Adapter = adapter;
            adapter.Monitors.Add(this);
            AllMonitors.Add(this);
        }


        ~Monitor()
        {
            Adapter.Monitors.Remove(this);
            AllMonitors.Remove(this);
            if (_pPhysicalMonitorArray != null && _pPhysicalMonitorArray.Length > 0)
                Dxva2.DestroyPhysicalMonitors((uint)_pPhysicalMonitorArray.Length, ref _pPhysicalMonitorArray);
        }
        private uint _flags;
        private Rect _monitorArea;
        private Rect _workArea;
        private int _displayFixedOutput;
        private int _displayFrequency;
        private int _displayFlags;
        private int _pelsWidth;
        private int _bitsPerPixel;
        private int _displayOrientation;
        private Point _position;
        private int _pelsHeight;
        private IntPtr hMonitor;
        private string _hKeyName;
        private string _manufacturerCode = "";
        private byte[] _edid;
        private string _productCode = "";
        private string _serial = "";
        private string _model = "";
        private string _serialNo = "";
        private Size _physicalSize;


        public void UpdateDevMode()
        {
            DEVMODE devmode = new DEVMODE(true);
            if (User32.EnumDisplaySettings(Adapter.DeviceName, -1, ref devmode))
            {
                if ((devmode.Fields & DM.Position) != 0) Position = new Point(devmode.Position.x, devmode.Position.y);
                if ((devmode.Fields & DM.DisplayOrientation) != 0) DisplayOrientation = devmode.DisplayOrientation;
                if ((devmode.Fields & DM.BitsPerPixel) != 0) BitsPerPixel = devmode.BitsPerPel;
                if ((devmode.Fields & DM.PelsWidth) != 0) PelsWidth = devmode.PelsWidth;
                if ((devmode.Fields & DM.PelsHeight) != 0) PelsHeight = devmode.PelsHeight;
                if ((devmode.Fields & DM.DisplayFlags) != 0) DisplayFlags = devmode.DisplayFlags;
                if ((devmode.Fields & DM.DisplayFrequency) != 0) DisplayFrequency = devmode.DisplayFrequency;
                if ((devmode.Fields & DM.DisplayFixedOutput) != 0) DisplayFixedOutput = devmode.DisplayFixedOutput;
            }

            // devmode.
        }

        public int PelsHeight
        {
            get { return _pelsHeight; }
            private set { SetProperty(ref _pelsHeight, value); }
        }

        public int DisplayFixedOutput
        {
            get { return _displayFixedOutput; }
            private set { SetProperty(ref _displayFixedOutput, value); }
        }

        public int DisplayFrequency
        {
            get { return _displayFrequency; }
            private set { SetProperty(ref _displayFrequency, value); }
        }

        public int DisplayFlags
        {
            get { return _displayFlags; }
            private set { SetProperty(ref _displayFlags, value); }
        }

        public int PelsWidth
        {
            get { return _pelsWidth; }
            private set { SetProperty(ref _pelsWidth, value); }
        }

        public int BitsPerPixel
        {
            get { return _bitsPerPixel; }
            private set { SetProperty(ref _bitsPerPixel, value); }
        }

        public int DisplayOrientation
        {
            get { return _displayOrientation; }
            private set { SetProperty(ref _displayOrientation, value); }
        }

        public Point Position
        {
            get { return _position; }
            private set { SetProperty(ref _position, value); }
        }

        public uint Flags
        {
            get { return _flags; }
            set { SetProperty(ref _flags, value); }
        }

        public Rect MonitorArea
        {
            get { return _monitorArea; }
            set { SetProperty(ref _monitorArea, value); }
        }

        public Rect WorkArea
        {
            get { return _workArea; }
            set { SetProperty(ref _workArea, value); }
        }

        public IntPtr HMonitor
        {
            get { return hMonitor; }
            set { SetProperty(ref hMonitor, value); }
        }
        public string HKeyName
        {
            get { return _hKeyName; }
            set { SetProperty(ref _hKeyName, value); }
        }

        public string ManufacturerCode
        {
            get { return _manufacturerCode; }
            private set { SetProperty(ref _manufacturerCode, value); }
        }

        public byte[] Edid
        {
            get { return _edid; }
            set { SetProperty(ref _edid, value); }
        }

        [DependsOn(nameof(Edid))]
        private void UpdateManufacturerCode()
        {
            String code;
            if (Edid.Length < 10) code = "XXX";
            else
            {
                code = "" + (char)(64 + ((Edid[8] >> 2) & 0x1F));
                code += (char)(64 + (((Edid[8] << 3) | (Edid[9] >> 5)) & 0x1F));
                code += (char)(64 + (Edid[9] & 0x1F));
            }
            ManufacturerCode = code;
        }

        public String ProductCode
        {
            get { return _productCode; }
            private set { SetProperty(ref _productCode, value); }
        }

        [DependsOn(nameof(Edid))]
        private void UpdateProductCode()
        {
            if (Edid.Length < 12) ProductCode = "0000";
            else ProductCode = (Edid[10] + (Edid[11] << 8)).ToString("X4");

        }

        public string Serial
        {
            get { return _serial; }
            private set { SetProperty(ref _serial, value); }
        }

        [DependsOn(nameof(Edid))]
        private void UpdateSerial()
        {
            if (Edid.Length < 16) Serial = "00000000";
            string serial = "";
            for (int i = 12; i <= 15; i++) serial = (Edid[i]).ToString("X2") + serial;
            Serial = serial;
        }

        public string Model
        {
            get { return _model; }
            private set { SetProperty(ref _model, value); }
        }

        [DependsOn(nameof(Edid))]
        private void UpdateModel() { Model = Block((char)0xFC); }
        public string SerialNo
        {
            get { return _serialNo; }
            private set { SetProperty(ref _serialNo, value); }
        }

        [DependsOn(nameof(Edid))]
        private void UpdateSerialNo() { SerialNo = Block((char)0xFF); }

        public Size PhysicalSize => _physicalSize;

        [DependsOn(nameof(Edid))]
        private void UpdatePhysicalSize()
        {
            if (Edid.Length > 68)
            {
                int w = ((Edid[68] & 0xF0) << 4) + Edid[66];
                int h = ((Edid[68] & 0x0F) << 8) + Edid[67];

                bool changed = false;

                if (_physicalSize.Width != w)
                {
                    _physicalSize.Width = w;
                    changed = true;
                }
                if (_physicalSize.Height != h)
                {
                    _physicalSize.Height = h;
                    changed = true;
                }
                if (changed) RaiseProperty(nameof(PhysicalSize));
            }
        }

        public string Block(char code)
        {
            for (int i = 54; i <= 108; i += 18)
            {
                if (i < Edid.Length && Edid[i] == 0 && Edid[i + 1] == 0 && Edid[i + 2] == 0 && Edid[i + 3] == code)
                {
                    string s = "";
                    for (int j = i + 5; j < i + 18; j++)
                    {
                        char c = (char)Edid[j];
                        if (c == (char)0x0A) break;
                        s += c;
                    }
                    return s;
                }
            }
            return "";
        }

        public void DetachFromDesktop()
        {
            DEVMODE devmode = new DEVMODE();
            devmode.Size = (short)Marshal.SizeOf(devmode);

            devmode.DeviceName = Adapter.DeviceName;
            devmode.PelsHeight = 0;
            devmode.PelsWidth = 0;
            devmode.Fields = DM.PelsWidth | DM.PelsHeight /*| DM.BitsPerPixel*/ | DM.Position
                        | DM.DisplayFrequency | DM.DisplayFlags;

            DISP_CHANGE ch = User32.ChangeDisplaySettingsEx(Adapter.DeviceName, ref devmode, IntPtr.Zero, ChangeDisplaySettingsFlags.CDS_UPDATEREGISTRY | ChangeDisplaySettingsFlags.CDS_NORESET, IntPtr.Zero);
            if (ch == DISP_CHANGE.Successful)
                ch = User32.ChangeDisplaySettingsEx(null, IntPtr.Zero, IntPtr.Zero, 0, IntPtr.Zero);
        }

        private IntPtr _hPhysical;
        private PHYSICAL_MONITOR[] _pPhysicalMonitorArray;
        public IntPtr HPhysical
        {
            get { return _hPhysical; }
            private set { SetProperty(ref _hPhysical, value); }
        }

        [DependsOn(nameof(HMonitor))]
        private void UpdateHPhysical()
        {
            uint pdwNumberOfPhysicalMonitors = 0;

            if (Dxva2.GetNumberOfPhysicalMonitorsFromHMONITOR(HMonitor, ref pdwNumberOfPhysicalMonitors))
            {
                _pPhysicalMonitorArray = new PHYSICAL_MONITOR[pdwNumberOfPhysicalMonitors];

                if (Dxva2.GetPhysicalMonitorsFromHMONITOR(HMonitor, pdwNumberOfPhysicalMonitors, _pPhysicalMonitorArray))
                    HPhysical = _pPhysicalMonitorArray[0].hPhysicalMonitor;
            }
            else HPhysical = IntPtr.Zero;
        }

        private double _deviceCapsHorzSize = 0;

        public double DeviceCapsHorzSize
        {
            get { return _deviceCapsHorzSize; }
            private set { SetProperty(ref _deviceCapsHorzSize, value); }
        }

        private double _deviceCapsVertSize = 0;
        public double DeviceCapsVertSize
        {
            get { return _deviceCapsVertSize; }
            private set { SetProperty(ref _deviceCapsVertSize, value); }
        }

        public void UpdateDeviceCaps()
        {
            IntPtr hdc = Gdi32.CreateDC("DISPLAY", Adapter.DeviceName, null, IntPtr.Zero);

            DeviceCapsHorzSize = Gdi32.GetDeviceCaps(hdc, DeviceCap.HORZSIZE);
            DeviceCapsVertSize = Gdi32.GetDeviceCaps(hdc, DeviceCap.VERTSIZE);

            // TODO : https://msdn.microsoft.com/en-us/library/windows/desktop/dd144877(v=vs.85).aspx

            Gdi32.DeleteDC(hdc);
        }

    }
}

