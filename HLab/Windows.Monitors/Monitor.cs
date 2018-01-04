using System;
using System.Collections.Generic;
using System.Linq;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading.Tasks;
using System.Windows;
using HLab.Mvvm.Observables;
using HLab.Notify;
using HLab.Windows.API;
using Microsoft.Win32;

namespace HLab.Windows.Monitors
{
    public class Monitor : NotifierObject
    {
        ~Monitor()
        {
            if (_pPhysicalMonitorArray != null && _pPhysicalMonitorArray.Length > 0)
                NativeMethods.DestroyPhysicalMonitors((uint)_pPhysicalMonitorArray.Length, ref _pPhysicalMonitorArray);
        }
        public MonitorsService Service
        {
            get => this.Get<MonitorsService>();
            private set => this.Set(value);
        }

        public string DeviceKey
        {
            get => this.Get<string>();
            internal set => this.Set(value);
        }
        public string DeviceId
        {
            get => this.Get<string>();
            internal set => this.Set(value);
        }

        public string DeviceString
        {
            get => this.Get<string>();
            internal set => this.Set(value ?? "");
        }

        [TriggedOn(nameof(DeviceId))]
        [TriggedOn(nameof(Service), "Devices", "Item", "DeviceId")]
        public ObservableFilter<DisplayDevice> Devices => this.Get(() => new ObservableFilter<DisplayDevice>()
            .AddFilter(a => a.DeviceId == DeviceId)
            .Link(Service.Devices)
        );

        [TriggedOn(nameof(Devices),"Item","AttachedToDesktop")]
        public DisplayDevice AttachedDevice => this.Get(() => Devices.FirstOrDefault(d => d.AttachedToDesktop));

        [TriggedOn(nameof(AttachedDevice),"Parent")]
        public DisplayDevice AttachedDisplay => this.Get(() => AttachedDevice?.Parent);

        public void Init(IntPtr hMonitor, NativeMethods.MONITORINFOEX mi)
        {
            Primary = mi.Flags == 1;
            MonitorArea = mi.Monitor;
            WorkArea = mi.WorkArea;

            HMonitor = hMonitor;
        }
        public Rect MonitorArea
        {
            get => this.Get<Rect>(); private set => this.Set(value);
        }

        public Rect WorkArea
        {
            get => this.Get<Rect>(); private set => this.Set(value);
        }

        public IntPtr HMonitor
        {
            get => this.Get<IntPtr>(); private set => this.Set(value);
        }
        public string HKeyName
        {
            get => this.Get<string>(); set => this.Set(value);
        }


        public bool AttachedToDesktop
        {
            get => this.Get<bool>();
            internal set => this.Set(value);
        }

        public bool Primary
        {
            get => this.Get(() => false);
            internal set
            {
                // Must remove old primary screen before setting this one
                if (value)
                {
                    foreach (Monitor monitor in Service.Monitors.Where(m => !m.Equals(this)))
                    {
                        monitor.Primary = false;
                    }
                }

                this.Set(value);
            }
        }
        public Edid Edid => this.Get<Edid>(() =>
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
                if (devInfo == IntPtr.Zero) return null;

                NativeMethods.SP_DEVINFO_DATA devInfoData = new NativeMethods.SP_DEVINFO_DATA(true);

                uint i = 0;

                do
                {
                    if (NativeMethods.SetupDiEnumDeviceInfo(devInfo, i, ref devInfoData))
                    {

                        IntPtr hEdidRegKey = NativeMethods.SetupDiOpenDevRegKey(devInfo, ref devInfoData,
                            NativeMethods.DICS_FLAG_GLOBAL, 0, NativeMethods.DIREG_DEV, NativeMethods.KEY_READ);

                        try
                        {
                            if (hEdidRegKey != IntPtr.Zero && (hEdidRegKey.ToInt32() != -1))
                            {
                                using (RegistryKey key = GetKeyFromPath(GetHKeyName(hEdidRegKey), 1))
                                {
                                    string id = ((string[])key.GetValue("HardwareID"))[0] + "\\" +
                                                key.GetValue("Driver");

                                    if (id == DeviceId)
                                    {
                                        HKeyName = GetHKeyName(hEdidRegKey);
                                        using (RegistryKey keyEdid = GetKeyFromPath(HKeyName))
                                        {
                                            return new Edid((byte[])keyEdid.GetValue("EDID"));
                                        }
                                    }
                                }
                            }
                        }
                        finally
                        {
                            NativeMethods.RegCloseKey(hEdidRegKey);
                        }
                    }
                    i++;
                } while (NativeMethods.ERROR_NO_MORE_ITEMS != NativeMethods.GetLastError());
            }
            finally
            {
                NativeMethods.SetupDiDestroyDeviceInfoList(devInfo);
            }
            return null;

        });


        private NativeMethods.PHYSICAL_MONITOR[] _pPhysicalMonitorArray;

        public Monitor(MonitorsService service)
        {
            Service = service;
            this.SubscribeNotifier();
        }

        [TriggedOn(nameof(HMonitor))]
        public IntPtr HPhysical => this.Get<IntPtr>(() =>
            {
                uint pdwNumberOfPhysicalMonitors = 0;

                if (!NativeMethods.GetNumberOfPhysicalMonitorsFromHMONITOR(HMonitor, ref pdwNumberOfPhysicalMonitors)) return IntPtr.Zero;

                _pPhysicalMonitorArray = new NativeMethods.PHYSICAL_MONITOR[pdwNumberOfPhysicalMonitors];

                if (!NativeMethods.GetPhysicalMonitorsFromHMONITOR(HMonitor, pdwNumberOfPhysicalMonitors, _pPhysicalMonitorArray)) return IntPtr.Zero;

                return _pPhysicalMonitorArray[0].hPhysicalMonitor;
            }
        );
        private enum DpiType
        {
            Effective = 0,
            Angular = 1,
            Raw = 2,
        }        //https://msdn.microsoft.com/en-us/library/windows/desktop/dn280510.aspx

        [TriggedOn(nameof(HMonitor))]
        public Vector EffectiveDpi => this.Get(() =>
        {
            GetDpiForMonitor(HMonitor, DpiType.Effective, out var x, out var y);
            return new Vector(x, y);
        });

        [TriggedOn(nameof(HMonitor))]
        public Vector AngularDpi => this.Get(() =>
        {
            GetDpiForMonitor(HMonitor, DpiType.Angular, out var x, out var y);
            return new Vector(x, y);
        });

        [TriggedOn(nameof(HMonitor))]
        public Vector RawDpi => this.Get(() =>
        {
            GetDpiForMonitor(HMonitor, DpiType.Raw, out var x, out var y);
            return new Vector(x, y);
        });

        [DllImport("Shcore.dll")]
        private static extern IntPtr GetDpiForMonitor([In]IntPtr hmonitor, [In]DpiType dpiType, [Out]out uint dpiX, [Out]out uint dpiY);

        //https://msdn.microsoft.com/fr-fr/library/windows/desktop/dn302060.aspx
        [TriggedOn(nameof(HMonitor))]
        public double ScaleFactor => this.Get(() =>
        {
            var factor = 100;
            NativeMethods.GetScaleFactorForMonitor(HMonitor, ref factor);
            return (double)factor / 100;
        });
        public int MonitorNo
        {
            get
            {
                var i = 1;
                foreach (var monitor in MonitorsService.D.Monitors.OrderBy(e => e.DeviceId.Split('\\').Last()))
                {
                    if (ReferenceEquals(monitor, this)) return i;
                    if(monitor.AttachedToDesktop) i++;
                }
                return 0;
            }
        }

        public override bool Equals(object obj) => obj is Monitor other ? DeviceId == other.DeviceId : base.Equals(obj);


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


    }
}
