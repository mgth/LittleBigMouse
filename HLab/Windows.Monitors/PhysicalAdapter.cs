using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using HLab.Mvvm.Observables;
using HLab.Notify;

namespace HLab.Windows.Monitors
{
    public class PhysicalAdapter : NotifierObject
    {
        public MonitorsService Service => this.Get(()=>MonitorsService.D);

        public string DeviceString
        {
            get => this.Get<string>();
            internal set => this.Set(value ?? "");
        }
        public string DeviceId
        {
            get => this.Get<string>();
            internal set => this.Set(value);
        }

        [TriggedOn(nameof(DeviceId))]
        [TriggedOn(nameof(Service),"Displays","Item","DeviceId")]
        public ObservableFilter<DisplayDevice> Displays => this.Get(() => new ObservableFilter<DisplayDevice>()
            .AddFilter(a => a.DeviceId == DeviceId)
            .Link(Service.Devices)
        );
    }
}
