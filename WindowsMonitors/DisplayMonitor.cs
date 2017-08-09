using System;
using System.Linq;
using System.Runtime.InteropServices;
using System.Windows;
using Erp.Notify;
using Microsoft.Win32;
using WinAPI;

namespace WindowsMonitors
{
    public class DisplayMonitor : DisplayDevice
    {
        public DisplayAdapter Adapter { get; private set; }
        public DisplayMonitor(DisplayAdapter adapter, NativeMethods.DISPLAY_DEVICE dev)
        {
            Init(adapter, dev);
        }
        public void Init(DisplayAdapter adapter, NativeMethods.DISPLAY_DEVICE dev)
        {
            using (this.Suspend())
            {
                Adapter = adapter;

                DeviceId = dev.DeviceID;
                DeviceKey = dev.DeviceKey;
                DeviceName = dev.DeviceName;
                DeviceString = dev.DeviceString;
                State = dev.StateFlags;

                UpdateDevMode();
                UpdateDeviceCaps();
                UpdateEdid();               
            }
        }

        public void Init(IntPtr hMonitor, NativeMethods.MONITORINFOEX mi)
        {
            AttachedToDesktop = true;

            Primary = mi.Flags;
            MonitorArea = mi.Monitor;
            WorkArea = mi.WorkArea;

            HMonitor = hMonitor;
        }

        public override bool Equals(Object obj)
        {
            if (obj == null) return false;
            var other = obj as DisplayMonitor;
            if (other == null) return base.Equals(obj);
            return DeviceId == other.DeviceId;
        }

        public override int GetHashCode()
        {
            return ("DisplayMonitor" + DeviceId).GetHashCode();
        }

        ~DisplayMonitor()
        {
            //AttachedMonitors.Remove(this); TODO : this is not thread safe
            if (_pPhysicalMonitorArray != null && _pPhysicalMonitorArray.Length > 0)
                NativeMethods.DestroyPhysicalMonitors((uint)_pPhysicalMonitorArray.Length, ref _pPhysicalMonitorArray);
        }


        public void UpdateDevMode()
        {
            NativeMethods.DEVMODE devmode = new NativeMethods.DEVMODE(true);
            if (NativeMethods.EnumDisplaySettings(Adapter.DeviceName, -1, ref devmode))
            {
                // Orientation should be set before any dimension
                if ((devmode.Fields & NativeMethods.DM.DisplayOrientation) != 0) DisplayOrientation = devmode.DisplayOrientation;

                if ((devmode.Fields & NativeMethods.DM.Position) != 0) Position = new Point(devmode.Position.x, devmode.Position.y);
                if ((devmode.Fields & NativeMethods.DM.BitsPerPixel) != 0) BitsPerPixel = devmode.BitsPerPel;
                if ((devmode.Fields & NativeMethods.DM.PelsWidth) != 0) PelsWidth = devmode.PelsWidth;
                if ((devmode.Fields & NativeMethods.DM.PelsHeight) != 0) PelsHeight = devmode.PelsHeight;
                if ((devmode.Fields & NativeMethods.DM.DisplayFlags) != 0) DisplayFlags = devmode.DisplayFlags;
                if ((devmode.Fields & NativeMethods.DM.DisplayFrequency) != 0) DisplayFrequency = devmode.DisplayFrequency;
                if ((devmode.Fields & NativeMethods.DM.DisplayFixedOutput) != 0) DisplayFixedOutput = devmode.DisplayFixedOutput;
            }
        }
        public bool AttachedToDesktop
        {
            get => this.Get<bool>();
            private set => this.Set(value);
        }

        [TriggedOn(nameof(State))]
        void UpdateFromState()
        {
            AttachedToDesktop = (State & NativeMethods.DisplayDeviceStateFlags.AttachedToDesktop) != 0;
        }


        public int PelsHeight
        {
            get => this.Get<int>();
            private set => this.Set(value);
        }
        //public int PelsHeight
        //{
        //    get { return Get<int>(); }
        //    private set { Set(value); }
        //}
        public int PelsWidth
        {
            get => this.Get<int>();
            private set => this.Set(value);
        }

        public int DisplayFixedOutput
        {
            get => this.Get<int>(); private set => this.Set(value);
        }

        public int DisplayFrequency
        {
            get => this.Get<int>(); private set => this.Set(value);
        }

        public int DisplayFlags
        {
            get => this.Get<int>(); private set => this.Set(value);
        }


        public int BitsPerPixel
        {
            get => this.Get<int>(); private set => this.Set(value);
        }

        public int DisplayOrientation
        {
            get => this.Get<int>(); set => this.Set(value);
        }

        public Point Position
        {
            get => this.Get<Point>(); set => this.Set(value);
        }

        public uint Primary
        {
            get => this.Get<uint>(); set
            {
                // Must remove old primary screen before setting this one
                if (value == 1)
                {
                    foreach (DisplayMonitor monitor in AttachedMonitors.Where(m => m != this))
                    {
                        monitor.Primary = 0;
                    }
                }

                this.Set(value);
            }
        }

        public Rect MonitorArea
        {
            get => this.Get<Rect>(); set => this.Set(value);
        }

        public Rect WorkArea
        {
            get => this.Get<Rect>(); set => this.Set(value);
        }

        public IntPtr HMonitor
        {
            get => this.Get<IntPtr>(); set => this.Set(value);
        }
        public string HKeyName
        {
            get => this.Get<string>(); set => this.Set(value);
        }

        public string ManufacturerCode
        {
            get => this.Get<string>(); private set => this.Set(value);
        }

        public byte[] Edid
        {
            get => this.Get<byte[]>(); private set => this.Set(value);
        }

        public void UpdateEdid()
        {
            IntPtr devInfo = NativeMethods.SetupDiGetClassDevsEx(
                ref NativeMethods.GUID_CLASS_MONITOR, //class GUID
                null, //enumerator
                IntPtr.Zero, //HWND
                NativeMethods.DIGCF_PRESENT | NativeMethods.DIGCF_PROFILE, // Primary //DIGCF_ALLCLASSES|
                IntPtr.Zero, // device info, create a new one.
                null, // machine name, local machine
                 IntPtr.Zero
            );// reserved

            if (devInfo == IntPtr.Zero)
                return;

            NativeMethods.SP_DEVINFO_DATA devInfoData = new NativeMethods.SP_DEVINFO_DATA(true);

            uint i = 0;

            do
            {
                if (NativeMethods.SetupDiEnumDeviceInfo(devInfo, i, ref devInfoData))
                {

                    IntPtr hEdidRegKey = NativeMethods.SetupDiOpenDevRegKey(devInfo, ref devInfoData,
                        NativeMethods.DICS_FLAG_GLOBAL, 0, NativeMethods.DIREG_DEV, NativeMethods.KEY_READ);

                    if (hEdidRegKey != IntPtr.Zero && (hEdidRegKey.ToInt32() != -1))
                    {
                        using (RegistryKey key = GetKeyFromPath(GetHKeyName(hEdidRegKey), 1))
                        {
                            string id = ((string[])key.GetValue("HardwareID"))[0] + "\\" + key.GetValue("Driver");

                            if (id == DeviceId)
                            {
                                HKeyName = GetHKeyName(hEdidRegKey);
                                using (RegistryKey keyEdid = GetKeyFromPath(HKeyName))
                                {
                                    Edid = (byte[])keyEdid.GetValue("EDID");
                                }
                                NativeMethods.RegCloseKey(hEdidRegKey);
                                return;
                            }
                        }
                        NativeMethods.RegCloseKey(hEdidRegKey);
                    }
                }
                i++;
            } while (NativeMethods.ERROR_NO_MORE_ITEMS != NativeMethods.GetLastError());

            NativeMethods.SetupDiDestroyDeviceInfoList(devInfo);
        }
        [TriggedOn(nameof(Edid))]
        private void UpdateManufacturerCode()
        {
            String code;
            if (Edid==null || Edid.Length < 10) code = "XXX";
            else
            {
                code = "" + (char)(64 + ((Edid[8] >> 2) & 0x1F));
                code += (char)(64 + (((Edid[8] << 3) | (Edid[9] >> 5)) & 0x1F));
                code += (char)(64 + (Edid[9] & 0x1F));
            }
            ManufacturerCode = code;
        }

        public string ProductCode
        {
            get => this.Get<string>(); private set => this.Set(value);
        }

        [TriggedOn(nameof(Edid))]
        private void UpdateProductCode()
        {
            if (Edid == null || Edid.Length < 12) ProductCode = "0000";
            else ProductCode = (Edid[10] + (Edid[11] << 8)).ToString("X4");

        }

        public string Serial
        {
            get => this.Get<string>(); private set => this.Set(value);
        }

        [TriggedOn(nameof(Edid))]
        private void UpdateSerial()
        {
            if (Edid == null || Edid.Length < 16)
            {
                Serial = "00000000";
                return;
            }
            string serial = "";
            for (int i = 12; i <= 15; i++) serial = (Edid[i]).ToString("X2") + serial;
            Serial = serial;
        }

        public string Model
        {
            get => this.Get<string>(); private set => this.Set(value);
        }

        [TriggedOn(nameof(Edid))]
        private void UpdateModel() { Model = Block((char)0xFC); }
        public string SerialNo
        {
            get => this.Get<string>(); private set => this.Set(value);
        }

        [TriggedOn(nameof(Edid))]
        private void UpdateSerialNo() { SerialNo = Block((char)0xFF); }

        public Size PhysicalSize
        {
            get => this.Get<Size>(); set => this.Set(value);
        }

        [TriggedOn(nameof(Edid))]
        private void UpdatePhysicalSize()
        {
            if (Edid != null && Edid.Length > 68)
            {
                int w = ((Edid[68] & 0xF0) << 4) + Edid[66];
                int h = ((Edid[68] & 0x0F) << 8) + Edid[67];

                PhysicalSize  = new Size(w,h);
            }
        }

        public string Block(char code)
        {
            if (Edid == null) return "";

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
        public void AttachToDesktop(bool primary, Rect area, int orientation, bool apply = true)
        {

            NativeMethods.DEVMODE devmode = new NativeMethods.DEVMODE(true);

            devmode.DeviceName = Adapter.DeviceName /*+ @"\Monitor0"*/;

            devmode.Position = new NativeMethods.POINTL { x = (int)area.X, y = (int)area.Y };
            devmode.Fields |= NativeMethods.DM.Position;

            devmode.PelsWidth = (int)area.Width;
            devmode.PelsHeight = (int)area.Height;
            devmode.Fields |= NativeMethods.DM.PelsHeight | NativeMethods.DM.PelsWidth;

            devmode.DisplayOrientation = orientation;
            devmode.Fields |= NativeMethods.DM.DisplayOrientation;

            devmode.BitsPerPel = 32;
            devmode.Fields |= NativeMethods.DM.BitsPerPixel;

            NativeMethods.ChangeDisplaySettingsFlags flag =
                NativeMethods.ChangeDisplaySettingsFlags.CDS_UPDATEREGISTRY |
                NativeMethods.ChangeDisplaySettingsFlags.CDS_NORESET;

            if (primary) flag |= NativeMethods.ChangeDisplaySettingsFlags.CDS_SET_PRIMARY;


            var ch = NativeMethods.ChangeDisplaySettingsEx(Adapter.DeviceName, ref devmode, IntPtr.Zero, flag, IntPtr.Zero);
            
            if (ch == NativeMethods.DISP_CHANGE.Successful && apply)
                ApplyDesktop();
        }

        public static void ApplyDesktop()
        {
            NativeMethods.ChangeDisplaySettingsEx(null, IntPtr.Zero, IntPtr.Zero, 0, IntPtr.Zero);
        }

        public void DetachFromDesktop(bool apply = true)
        {
            NativeMethods.DEVMODE devmode = new NativeMethods.DEVMODE();
            devmode.Size = (short)Marshal.SizeOf(devmode);

            devmode.DeviceName = Adapter.DeviceName;
            devmode.PelsHeight = 0;
            devmode.PelsWidth = 0;
            devmode.Fields = NativeMethods.DM.PelsWidth | NativeMethods.DM.PelsHeight /*| DM.BitsPerPixel*/ | NativeMethods.DM.Position
                        | NativeMethods.DM.DisplayFrequency | NativeMethods.DM.DisplayFlags;

            NativeMethods.DISP_CHANGE ch = NativeMethods.ChangeDisplaySettingsEx(Adapter.DeviceName, ref devmode, IntPtr.Zero, NativeMethods.ChangeDisplaySettingsFlags.CDS_UPDATEREGISTRY | NativeMethods.ChangeDisplaySettingsFlags.CDS_NORESET, IntPtr.Zero);
            if (ch == NativeMethods.DISP_CHANGE.Successful && apply)
                ApplyDesktop();
        }

        private NativeMethods.PHYSICAL_MONITOR[] _pPhysicalMonitorArray;
        public IntPtr HPhysical
        {
            get => this.Get<IntPtr>(); private set => this.Set(value);
        }

        [TriggedOn(nameof(HMonitor))]
        private void UpdateHPhysical()
        {
            uint pdwNumberOfPhysicalMonitors = 0;

            if (NativeMethods.GetNumberOfPhysicalMonitorsFromHMONITOR(HMonitor, ref pdwNumberOfPhysicalMonitors))
            {
                _pPhysicalMonitorArray = new NativeMethods.PHYSICAL_MONITOR[pdwNumberOfPhysicalMonitors];

                if (NativeMethods.GetPhysicalMonitorsFromHMONITOR(HMonitor, pdwNumberOfPhysicalMonitors, _pPhysicalMonitorArray))
                    HPhysical = _pPhysicalMonitorArray[0].hPhysicalMonitor;
            }
            else HPhysical = IntPtr.Zero;
        }


        public double DeviceCapsHorzSize
        {
            get => this.Get<double>(); private set => this.Set(value);
        }

        public double DeviceCapsVertSize
        {
            get => this.Get<double>(); private set => this.Set(value);
        }

        public void UpdateDeviceCaps()
        {
            IntPtr hdc = NativeMethods.CreateDC("DISPLAY", Adapter.DeviceName, null, IntPtr.Zero);

            DeviceCapsHorzSize = NativeMethods.GetDeviceCaps(hdc, NativeMethods.DeviceCap.HORZSIZE);
            DeviceCapsVertSize = NativeMethods.GetDeviceCaps(hdc, NativeMethods.DeviceCap.VERTSIZE);

            // TODO : https://msdn.microsoft.com/en-us/library/windows/desktop/dd144877(v=vs.85).aspx

            NativeMethods.DeleteDC(hdc);
        }

        public Vector EffectiveDpi
        {
            get => this.Get<Vector>(); private set => this.Set(value);
        }

        public Vector AngularDpi
        {
            get => this.Get<Vector>(); private set => this.Set(value);
        }

        public Vector RawDpi
        {
            get => this.Get<Vector>(); private set => this.Set(value);
        }

        private enum DpiType
        {
            Effective = 0,
            Angular = 1,
            Raw = 2,
        }        //https://msdn.microsoft.com/en-us/library/windows/desktop/dn280510.aspx
        [DllImport("Shcore.dll")]
        private static extern IntPtr GetDpiForMonitor([In]IntPtr hmonitor, [In]DpiType dpiType, [Out]out uint dpiX, [Out]out uint dpiY);

        [TriggedOn(nameof(HMonitor))]
        private void UpdateDpiForMonitor()
        {
            uint x;
            uint y;
            GetDpiForMonitor(HMonitor, DpiType.Effective, out x, out y);
            EffectiveDpi = new Vector(x, y);
            GetDpiForMonitor(HMonitor, DpiType.Angular, out x, out y);
            AngularDpi = new Vector(x, y);
            GetDpiForMonitor(HMonitor, DpiType.Raw, out x, out y);
            RawDpi = new Vector(x, y);
        }

        public double ScaleFactor
        {
            get => this.Get<double>(() => 1); private set => this.Set(value);
        }

        [TriggedOn(nameof(HMonitor))]
        public void UpdateScaleFactor()
        {
            int factor = 100;
            NativeMethods.GetScaleFactorForMonitor(HMonitor, ref factor);
            ScaleFactor = ((double)factor) / 100;
        }

    }
}
