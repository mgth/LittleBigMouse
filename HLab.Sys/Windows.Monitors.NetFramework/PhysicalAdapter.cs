using HLab.Base;
using HLab.DependencyInjection.Annotations;
using HLab.Mvvm.Observables;
using HLab.Notify.Annotations;
using HLab.Notify.PropertyChanged;
using Newtonsoft.Json;

namespace HLab.Windows.Monitors
{
    public class PhysicalAdapter : N<PhysicalAdapter>
    {
        public PhysicalAdapter(string deviceId, IMonitorsService service)
        {
            DeviceId = deviceId;
            MonitorsService = service;

            Initialize();
        }

        [JsonIgnore]
        public IMonitorsService MonitorsService { get; }
        public string DeviceId { get; }

        public string DeviceString
        {
            get => _deviceString.Get();
            internal set => _deviceString.Set(value ?? "");
        }
        private readonly IProperty<string> _deviceString = H.Property<string>();


        [TriggerOn(nameof(DeviceId))]
        [TriggerOn(nameof(MonitorsService), "Devices", "Item", "DeviceId")]
        public IObservableFilter<DisplayDevice> Displays { get; } = H.Filter<DisplayDevice>( (e,c)=> c
            .AddFilter(a => a.DeviceId == e.DeviceId)
            .Link(() => e.MonitorsService.Devices));
    }
}
