using System;
using System.Collections;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.ComponentModel;
using System.Diagnostics;
using System.Linq;
using System.Runtime.CompilerServices;
using System.Text;
using System.Threading.Tasks;
using WindowsMonitors.Annotations;
using Erp.Base;
using Erp.Mvvm;
using Erp.Notify;
using Microsoft.Win32;
using WinAPI;

namespace WindowsMonitors
{
    public class MonitorsService : Singleton<MonitorsService>, INotifyPropertyChanged
    {

        public event EventHandler DevicesUpdated;

        private MonitorsService()
        {
            SystemEvents.DisplaySettingsChanged += (sender, eventArgs) => UpdateDevices();
            this.Subscribe();
        }


        //public ObservableCollection<DisplayMonitor> AttachedMonitors => this.GetHashCode();
        //public ObservableCollection<DisplayMonitor> UnattachedMonitors { get; } = new ObservableCollection<DisplayMonitor>();

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
