using DynamicData;
using Newtonsoft.Json;
using ReactiveUI;
using System.Reactive.Linq;

namespace HLab.Sys.Windows.Monitors
{
    public class PhysicalAdapter : ReactiveObject
    {
        public PhysicalAdapter(string deviceId)
        {
            DeviceId = deviceId;
            //MonitorsService = service;

            // TODO 
            //Displays = service
            //    .Devices
            //    .Connect()
            //    .Filter(e => e.DeviceId == deviceId)
            //    .ObserveOn(RxApp.MainThreadScheduler)
            //    .AsObservableCache();
        }

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
