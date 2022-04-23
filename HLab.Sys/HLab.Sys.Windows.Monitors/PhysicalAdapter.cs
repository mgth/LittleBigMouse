using HLab.Notify.Annotations;
using HLab.Notify.PropertyChanged;
using Newtonsoft.Json;

namespace HLab.Sys.Windows.Monitors
{
    using H = H<PhysicalAdapter>;

    public class PhysicalAdapter : NotifierBase
    {
        public PhysicalAdapter(string deviceId, IMonitorsService service)
        {
            DeviceId = deviceId;
            MonitorsService = service;

            H.Initialize(this);
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


        public IObservableFilter<DisplayDevice> Displays { get; } = H.Filter<DisplayDevice>( c=> c
            .On(e => e.DeviceId)
            .On(e => e.MonitorsService.Devices.Item().DeviceId)
            .Update()
            .AddFilter((e,a) => a.DeviceId == e.DeviceId)
            .Link(e => e.MonitorsService.Devices));
    }
}
