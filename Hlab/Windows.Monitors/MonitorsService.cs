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
using System.Diagnostics;
using System.Linq;
using Hlab.Base;
using Hlab.Mvvm.Observables;
using Hlab.Notify;
using Microsoft.Win32;
using WinAPI;

namespace HLab.Windows.Monitors
{
    public class MonitorsService : Singleton<MonitorsService>, INotifyPropertyChanged
    {

        public event EventHandler DevicesUpdated;

        private MonitorsService()
        {
            SystemEvents.DisplaySettingsChanged += (sender, eventArgs) => UpdateDevices();
            this.Subscribe();
        }


        public ObservableCollectionNotifier<DisplayAdapter> Adapters => this.Get(() => new ObservableCollectionNotifier<DisplayAdapter>());
        public ObservableCollectionNotifier<DisplayMonitor> Monitors => this.Get(() => new ObservableCollectionNotifier<DisplayMonitor>());

        [TriggedOn(nameof(Monitors),"Item","AttachedToDesktop")]
        public ObservableFilter<DisplayMonitor> AttachedMonitors => this.Get(()=> new ObservableFilter<DisplayMonitor>()
            .AddFilter(m => m.AttachedToDesktop)
            .Link(Monitors)
        );

        [TriggedOn(nameof(Monitors), "Item","AttachedToDesktop")]
        public ObservableFilter<DisplayMonitor> UnattachedMonitors => this.Get(()=> new ObservableFilter<DisplayMonitor>()
            .AddFilter(m => !m.AttachedToDesktop)
            .Link(Monitors)
        );




        public void UpdateDevices()
        {
            List<DisplayAdapter> oldAdapters = Adapters.ToList();
            List<DisplayMonitor> oldMonitors = Monitors.ToList();

            NativeMethods.DISPLAY_DEVICE dev = new NativeMethods.DISPLAY_DEVICE(true);
            uint i = 0;

            while (NativeMethods.EnumDisplayDevices(null, i++, ref dev, 0))
            {
                var adapter = oldAdapters.FirstOrDefault(a => a.DeviceName == dev.DeviceName);
                if (adapter != null)
                {
                    oldAdapters.Remove(adapter);
                    adapter.Init(dev,oldMonitors);
                }
                else
                {
                    adapter = new DisplayAdapter();
                    adapter.Init(dev,oldMonitors);
                    Adapters.Add(adapter);                    
                }
            }

            var w = new Stopwatch();
            w.Start();

            NativeMethods.EnumDisplayMonitors(IntPtr.Zero, IntPtr.Zero,
                delegate (IntPtr hMonitor, IntPtr hdcMonitor, ref NativeMethods.RECT lprcMonitor, IntPtr dwData)
                {
                    var mi = new NativeMethods.MONITORINFOEX(true);
                    var success = NativeMethods.GetMonitorInfo(hMonitor, ref mi);
                    if (!success) return true;

                    IList monitors = AttachedMonitors.Where(d => d.Adapter.DeviceName == mi.DeviceName).ToList();
                    foreach (DisplayMonitor monitor in monitors)
                    {
                        monitor.Init(hMonitor, mi);
                        monitor.Timing = w.ElapsedMilliseconds;
                        w.Restart();
                    }

                    return true;
                }, IntPtr.Zero);

            DevicesUpdated?.Invoke(this, new EventArgs());

            foreach (var monitor in Monitors)
            {
                Debug.Print(monitor.DeviceName + "\t" + monitor.Edid.Model + "\t" + monitor.Timing);
            }
        }

        public event PropertyChangedEventHandler PropertyChanged
        {
            add => this.Add(value);
            remove => this.Remove(value);
        }

    }
}
