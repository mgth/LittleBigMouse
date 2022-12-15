using DynamicData;
using HLab.Sys.Windows.API;
using Newtonsoft.Json;
using ReactiveUI;

namespace HLab.Sys.Windows.Monitors
{
    public class PhysicalAdapter : ReactiveObject
    {
        public PhysicalAdapter(string deviceId, IMonitorsService service)
        {
            DeviceId = deviceId;
            MonitorsService = service;

            Displays = service.Devices.Connect().Filter(e => e.DeviceId == deviceId).AsObservableCache();
        }

        [JsonIgnore]
        public IMonitorsService MonitorsService { get; }
        public string DeviceId { get; }

        public string DeviceString
        {
            get => _deviceString;
            internal set => this.RaiseAndSetIfChanged(ref _deviceString, value??"");
        }
        string _deviceString;


        public IObservableCache<DisplayDevice,string> Displays { get; }
    }
}
