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
using System.Collections;
using System.Collections.Generic;
using System.ComponentModel;
using System.Linq;
using System.Runtime.InteropServices;
using System.Windows;
using HLab.Base;
using HLab.DependencyInjection.Annotations;
using HLab.Mvvm.Observables;
using HLab.Notify.Annotations;
using HLab.Notify.PropertyChanged;
using HLab.Windows.API;

namespace HLab.Windows.Monitors
{
    public interface IMonitorsService
    {
        ObservableCollectionSafe<Monitor> Monitors { get; }
        ObservableCollectionSafe<DisplayDevice> Devices { get; }
        IObservableFilter<Monitor> AttachedMonitors { get; }

        void DetachFromDesktop(string deviceName, bool apply = true);
        void AttachToDesktop(string deviceName, bool primary, Rect area, int orientation, bool apply = true);
        void ApplyDesktop();
        void UpdateDevices();
    }

    [Export(typeof(IMonitorsService)),Singleton]
    public class MonitorsService : N<MonitorsService>, INotifyPropertyChanged, IMonitorsService
    {

        public event EventHandler DevicesUpdated;

        private readonly Window _displayChanges;

        public MonitorsService()
        {

            Initialize();
            UpdateDevices();

            _displayChanges = new DispalyChangesView(this);

            _displayChanges.Show();
            _displayChanges.Hide();

        }

        public ObservableCollectionSafe<PhysicalAdapter> Adapters { get; } = new ObservableCollectionSafe<PhysicalAdapter>();
        public ObservableCollectionSafe<DisplayDevice> Devices { get; } = new ObservableCollectionSafe<DisplayDevice>();
        public ObservableCollectionSafe<Monitor> Monitors { get; } = new ObservableCollectionSafe<Monitor>();


        [TriggerOn(nameof(Monitors),"Item","AttachedToDesktop")]
        public IObservableFilter<Monitor> AttachedMonitors { get; }
            = new ObservableFilter<MonitorsService, Monitor>((e, c) => c
                .AddFilter(m => m.AttachedToDesktop)
                .Link(() => e.Monitors)
            );


        [TriggerOn(nameof(Monitors),"Item","AttachedToDesktop")]
        public IObservableFilter<Monitor> UnattachedMonitors { get; }
            = new ObservableFilter<MonitorsService, Monitor>((e, c) => c
                .AddFilter(m => !m.AttachedToDesktop)
                .Link(() => e.Monitors)
            );


        public Monitor GetOrAddMonitor(string deviceId,Func<Monitor> get)
        {
            var monitor = Monitors.FirstOrDefault(m => m.DeviceId == deviceId);
            if (monitor == null)
            {
                monitor = get();
                Monitors.Add(monitor);
            }
            return monitor;
        }

        public PhysicalAdapter GetOrAddAdapter(string deviceId, Func<PhysicalAdapter> get)
        {
            var adapter = Adapters.FirstOrDefault(m => m.DeviceId == deviceId);
            if (adapter == null)
            {
                adapter = get();
                Adapters.Add(adapter);
            }
            return adapter;
        }


        public void UpdateDevices()
        {
            List<DisplayDevice> oldDevices = Devices.ToList();

            var device = new DisplayDevice(this);
            device.Init(null,new NativeMethods.DISPLAY_DEVICE(true){DeviceID = "ROOT",DeviceName = null}, oldDevices );

            foreach (var d in oldDevices)
            {
                var monitor = Monitors.FirstOrDefault(m => m.AttachedDevice == d);
                if (monitor!=null) Monitors.Remove(monitor);
                Devices.Remove(d);
            }

            AttachedMonitors.OnTrigged();
            foreach (var m in AttachedMonitors) m.Devices.OnTrigged();


            NativeMethods.EnumDisplayMonitors(IntPtr.Zero, IntPtr.Zero,
                delegate (IntPtr hMonitor, IntPtr hdcMonitor, ref NativeMethods.RECT lprcMonitor, IntPtr dwData)
                {
                    var mi = new NativeMethods.MONITORINFOEX(true);
                    var success = NativeMethods.GetMonitorInfo(hMonitor, ref mi);
                    if (success)
                    {
                        IList monitors = AttachedMonitors.Where(d => d.AttachedDisplay?.DeviceName == mi.DeviceName).ToList();
                        foreach (Monitor monitor in monitors)
                        {
                            monitor.SetMonitorInfoEx(mi);
                            monitor.UpdateDpi(hMonitor);
                        }
                    }

                    return true; // Continue
                }, IntPtr.Zero);

            DevicesUpdated?.Invoke(this, new EventArgs());
        }

        public void AttachToDesktop(string deviceName, bool primary, Rect area, int orientation, bool apply = true)
        {
            var devmode = new NativeMethods.DEVMODE(true)
            {
                DeviceName = deviceName,
                Position = new NativeMethods.POINTL { x = (int)area.X, y = (int)area.Y },
                PelsWidth = (int)area.Width,
                PelsHeight = (int)area.Height,
                DisplayOrientation = orientation,
                BitsPerPel = 32,
                Fields = NativeMethods.DM.Position | NativeMethods.DM.PelsHeight | NativeMethods.DM.PelsWidth | NativeMethods.DM.DisplayOrientation | NativeMethods.DM.BitsPerPixel
            };

            var flag =
                NativeMethods.ChangeDisplaySettingsFlags.CDS_UPDATEREGISTRY |
                NativeMethods.ChangeDisplaySettingsFlags.CDS_NORESET;

            if (primary) flag |= NativeMethods.ChangeDisplaySettingsFlags.CDS_SET_PRIMARY;


            var ch = NativeMethods.ChangeDisplaySettingsEx(deviceName, ref devmode, IntPtr.Zero, flag, IntPtr.Zero);

            if (ch == NativeMethods.DISP_CHANGE.Successful && apply)
                ApplyDesktop();
        }

        public void DetachFromDesktop(string deviceName, bool apply = true)
        {
            NativeMethods.DEVMODE devmode = new NativeMethods.DEVMODE();
            devmode.Size = (short)Marshal.SizeOf(devmode);

            devmode.DeviceName = deviceName;
            devmode.PelsHeight = 0;
            devmode.PelsWidth = 0;
            devmode.Fields = NativeMethods.DM.PelsWidth | NativeMethods.DM.PelsHeight /*| DM.BitsPerPixel*/ | NativeMethods.DM.Position
                             | NativeMethods.DM.DisplayFrequency | NativeMethods.DM.DisplayFlags;

            var ch = NativeMethods.ChangeDisplaySettingsEx(deviceName, ref devmode, IntPtr.Zero, NativeMethods.ChangeDisplaySettingsFlags.CDS_UPDATEREGISTRY | NativeMethods.ChangeDisplaySettingsFlags.CDS_NORESET, IntPtr.Zero);
            if (ch == NativeMethods.DISP_CHANGE.Successful && apply)
                ApplyDesktop();
        }


        public void ApplyDesktop()
        {
            NativeMethods.ChangeDisplaySettingsEx(null, IntPtr.Zero, IntPtr.Zero, 0, IntPtr.Zero);
        }
    }
}
