using System;
using System.Collections.Generic;
using System.Linq;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading.Tasks;
using System.Windows;
using NotifyChange;
using WinAPI;

namespace WindowsMonitors
{
    public class DisplayMonitor : DisplayDevice
    {
        public DisplayAdapter Adapter { get; }
        public DisplayMonitor(DisplayAdapter adapter, NativeMethods.DISPLAY_DEVICE dev)
        {
            Adapter = adapter;
            Init(dev);
            adapter.Monitors.Add(this);
        }
        public void Init(NativeMethods.DISPLAY_DEVICE dev)
        {
            DeviceId = dev.DeviceID;
            DeviceKey = dev.DeviceKey;
            DeviceName = dev.DeviceName;
            DeviceString = dev.DeviceString;
            State = dev.StateFlags;

            UpdateDevMode();
            UpdateDeviceCaps();
        }

        public void Init(IntPtr hMonitor, NativeMethods.MONITORINFOEX mi)
        {
            Primary = mi.Flags;
            MonitorArea = mi.Monitor;
            WorkArea = mi.WorkArea;

            HMonitor = hMonitor;
        }


        ~DisplayMonitor()
        {
            Adapter.Monitors.Remove(this);
            //AllMonitors.Remove(this); TODO : this is not thread safe
            if (_pPhysicalMonitorArray != null && _pPhysicalMonitorArray.Length > 0)
                NativeMethods.DestroyPhysicalMonitors((uint)_pPhysicalMonitorArray.Length, ref _pPhysicalMonitorArray);
        }
        private uint _primary;
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
        private IntPtr _hMonitor;
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
            NativeMethods.DEVMODE devmode = new NativeMethods.DEVMODE(true);
            if (NativeMethods.EnumDisplaySettings(Adapter.DeviceName, -1, ref devmode))
            {
                if ((devmode.Fields & NativeMethods.DM.Position) != 0) Position = new Point(devmode.Position.x, devmode.Position.y);
                if ((devmode.Fields & NativeMethods.DM.DisplayOrientation) != 0) DisplayOrientation = devmode.DisplayOrientation;
                if ((devmode.Fields & NativeMethods.DM.BitsPerPixel) != 0) BitsPerPixel = devmode.BitsPerPel;
                if ((devmode.Fields & NativeMethods.DM.PelsWidth) != 0) PelsWidth = devmode.PelsWidth;
                if ((devmode.Fields & NativeMethods.DM.PelsHeight) != 0) PelsHeight = devmode.PelsHeight;
                if ((devmode.Fields & NativeMethods.DM.DisplayFlags) != 0) DisplayFlags = devmode.DisplayFlags;
                if ((devmode.Fields & NativeMethods.DM.DisplayFrequency) != 0) DisplayFrequency = devmode.DisplayFrequency;
                if ((devmode.Fields & NativeMethods.DM.DisplayFixedOutput) != 0) DisplayFixedOutput = devmode.DisplayFixedOutput;
            }
        }

        public int PelsHeight
        {
            get { return _pelsHeight; }
            private set { SetProperty(ref _pelsHeight, value); }
        }
        public int PelsWidth
        {
            get { return _pelsWidth; }
            private set { SetProperty(ref _pelsWidth, value); }
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

        public uint Primary
        {
            get { return _primary; }
            set
            {
                // Must remove old primary screen before setting this one
                if (value == 1)
                {
                    foreach (DisplayMonitor monitor in AllMonitors.Where(m => m!=this))
                    {
                        monitor.Primary = 0;
                    }
                }

                SetProperty(ref _primary, value);
            }
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
            get { return _hMonitor; }
            set { SetProperty(ref _hMonitor, value); }
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
            NativeMethods.DEVMODE devmode = new NativeMethods.DEVMODE();
            devmode.Size = (short)Marshal.SizeOf(devmode);

            devmode.DeviceName = Adapter.DeviceName;
            devmode.PelsHeight = 0;
            devmode.PelsWidth = 0;
            devmode.Fields = NativeMethods.DM.PelsWidth | NativeMethods.DM.PelsHeight /*| DM.BitsPerPixel*/ | NativeMethods.DM.Position
                        | NativeMethods.DM.DisplayFrequency | NativeMethods.DM.DisplayFlags;

            NativeMethods.DISP_CHANGE ch = NativeMethods.ChangeDisplaySettingsEx(Adapter.DeviceName, ref devmode, IntPtr.Zero, NativeMethods.ChangeDisplaySettingsFlags.CDS_UPDATEREGISTRY | NativeMethods.ChangeDisplaySettingsFlags.CDS_NORESET, IntPtr.Zero);
            if (ch == NativeMethods.DISP_CHANGE.Successful)
                ch = NativeMethods.ChangeDisplaySettingsEx(null, IntPtr.Zero, IntPtr.Zero, 0, IntPtr.Zero);
        }

        private IntPtr _hPhysical;
        private NativeMethods.PHYSICAL_MONITOR[] _pPhysicalMonitorArray;
        public IntPtr HPhysical
        {
            get { return _hPhysical; }
            private set { SetProperty(ref _hPhysical, value); }
        }

        [DependsOn(nameof(HMonitor))]
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
            IntPtr hdc = NativeMethods.CreateDC("DISPLAY", Adapter.DeviceName, null, IntPtr.Zero);

            DeviceCapsHorzSize = NativeMethods.GetDeviceCaps(hdc, NativeMethods.DeviceCap.HORZSIZE);
            DeviceCapsVertSize = NativeMethods.GetDeviceCaps(hdc, NativeMethods.DeviceCap.VERTSIZE);

            // TODO : https://msdn.microsoft.com/en-us/library/windows/desktop/dd144877(v=vs.85).aspx

            NativeMethods.DeleteDC(hdc);
        }

        private Vector _effectiveDpi = new Vector();
        public Vector EffectiveDpi
        {
            get { return _effectiveDpi; }
            private set { SetProperty(ref _effectiveDpi, value); }
        }
        private Vector _angularDpi = new Vector();
        public Vector AngularDpi
        {
            get { return _angularDpi; }
            private set { SetProperty(ref _angularDpi, value); }
        }
        private Vector _rawDpi = new Vector();
        public Vector RawDpi
        {
            get { return _rawDpi; }
            private set { SetProperty(ref _rawDpi, value); }
        }

        private enum DpiType
        {
            Effective = 0,
            Angular = 1,
            Raw = 2,
        }        //https://msdn.microsoft.com/en-us/library/windows/desktop/dn280510.aspx
        [DllImport("Shcore.dll")]
        private static extern IntPtr GetDpiForMonitor([In]IntPtr hmonitor, [In]DpiType dpiType, [Out]out uint dpiX, [Out]out uint dpiY);

        [DependsOn(nameof(HMonitor))]
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

        private double _scaleFactor = 1;
        public double ScaleFactor
        {
            get { return _scaleFactor; }
            private set { SetProperty(ref _scaleFactor, value); }
        }

        [DependsOn(nameof(HMonitor))]
        public void UpdateScaleFactor()
        {
            int factor = 100;
            NativeMethods.GetScaleFactorForMonitor(HMonitor, ref factor);
            ScaleFactor = ((double)factor) / 100;
        }

    }
}
