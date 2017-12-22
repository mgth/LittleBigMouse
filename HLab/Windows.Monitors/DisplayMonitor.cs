/*
  HLab.Windows.Monitors
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

  This file is part of HLab.Windows.Monitors.

    HLab.Windows.Monitors is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    HLab.Windows.Monitors is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/
using System;
using System.Linq;
using System.Runtime.InteropServices;
using System.Runtime.Remoting.Messaging;
using System.Windows;
using HLab.Notify;
using HLab.Windows.API;
using Microsoft.Win32;

namespace HLab.Windows.Monitors
{
    public class DisplayMonitor : DisplayDevice
    {
        public DisplayAdapter Adapter { get; private set; }

        public DisplayMonitor()
        {
            this.Subscribe();
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
            }
        }

        public void Init(IntPtr hMonitor, NativeMethods.MONITORINFOEX mi)
        {
            Primary = mi.Flags == 1;
            MonitorArea = mi.Monitor;
            WorkArea = mi.WorkArea;

            HMonitor = hMonitor;
        }

        public override bool Equals(object obj)=> obj is DisplayMonitor other ? DeviceName == other.DeviceName : base.Equals(obj);
        

        public override int GetHashCode()
        {
            return ("DisplayMonitor" + DeviceId).GetHashCode();
        }

        ~DisplayMonitor()
        {
            if (_pPhysicalMonitorArray != null && _pPhysicalMonitorArray.Length > 0)
                NativeMethods.DestroyPhysicalMonitors((uint)_pPhysicalMonitorArray.Length, ref _pPhysicalMonitorArray);
        }


        [TriggedOn(nameof(State))]
        public bool AttachedToDesktop => this.Get(()=> (State & NativeMethods.DisplayDeviceStateFlags.AttachedToDesktop) != 0);

        public int MonitorNo
        {
            get
            {
                var i = 1;
                foreach (var monitor in MonitorsService.D.AttachedMonitors.OrderBy(e => e.DeviceId.Split('\\').Last()))
                {
                    if(ReferenceEquals(monitor,this) ) return i;
                    i++;
                }
                return 0;
            }
        }


        public bool Primary
        {
            get => this.Get(()=>false);
            internal set
            {
                // Must remove old primary screen before setting this one
                if (value)
                {
                    foreach (DisplayMonitor monitor in MonitorsService.D.Monitors.Where(m => !m.Equals(this) ))
                    {
                        monitor.Primary = false;
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
                                    string id = ((string[]) key.GetValue("HardwareID"))[0] + "\\" +
                                                key.GetValue("Driver");

                                    if (id == DeviceId)
                                    {
                                        HKeyName = GetHKeyName(hEdidRegKey);
                                        using (RegistryKey keyEdid = GetKeyFromPath(HKeyName))
                                        {
                                            return new Edid((byte[]) keyEdid.GetValue("EDID"));
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


        public void AttachToDesktop(bool primary, Rect area, int orientation, bool apply = true)
        {

            var devmode = new NativeMethods.DEVMODE(true)
            {
                DeviceName = Adapter.DeviceName,
                Position = new NativeMethods.POINTL {x = (int) area.X, y = (int) area.Y}
            };


            devmode.Fields |= NativeMethods.DM.Position;

            devmode.PelsWidth = (int)area.Width;
            devmode.PelsHeight = (int)area.Height;
            devmode.Fields |= NativeMethods.DM.PelsHeight | NativeMethods.DM.PelsWidth;

            devmode.DisplayOrientation = orientation;
            devmode.Fields |= NativeMethods.DM.DisplayOrientation;

            devmode.BitsPerPel = 32;
            devmode.Fields |= NativeMethods.DM.BitsPerPixel;

            var flag =
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

            var ch = NativeMethods.ChangeDisplaySettingsEx(Adapter.DeviceName, ref devmode, IntPtr.Zero, NativeMethods.ChangeDisplaySettingsFlags.CDS_UPDATEREGISTRY | NativeMethods.ChangeDisplaySettingsFlags.CDS_NORESET, IntPtr.Zero);
            if (ch == NativeMethods.DISP_CHANGE.Successful && apply)
                ApplyDesktop();
        }

        private NativeMethods.PHYSICAL_MONITOR[] _pPhysicalMonitorArray;


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
    }
}
