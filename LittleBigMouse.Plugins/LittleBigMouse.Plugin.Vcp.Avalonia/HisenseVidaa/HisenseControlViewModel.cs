#nullable enable
using System.Windows.Input;
using Avalonia.Threading;
using LittleBigMouse.DisplayLayout.Monitors;
using ReactiveUI;

namespace LittleBigMouse.Plugin.Vcp.Avalonia.HisenseVidaa;

public sealed class HisenseControlViewModel : ReactiveObject, IDisposable
{
    readonly IHisenseVidaaService? _service;
    readonly Queue<VidaaTrafficMessage> _trafficMessages = new();
    CancellationTokenSource? _trafficCancellation;
    CancellationTokenSource? _volumeSetCancellation;
    CancellationTokenSource? _volumeRefreshCancellation;
    CancellationTokenSource? _laserSetCancellation;
    PhysicalMonitor? _monitor; string _ip="", _mac="", _controllerMac="", _certificatePath="", _uuid="", _pin="", _keyMacro="KEY_BRIGHTNESSUP", _capturedTraffic="", _pictureSettingsCatalog="", _status="", _laserAction="changelaserluminance"; int _pictureMenuId=2, _pictureMenuValue=5; double _volume, _laserLevel=5; bool _visible, _busy, _paired, _setup, _listening, _applyingVolume;
    public bool IsVisible { get=>_visible; private set=>this.RaiseAndSetIfChanged(ref _visible,value); }
    public bool Busy { get=>_busy; private set=>this.RaiseAndSetIfChanged(ref _busy,value); }
    public string IpAddress { get=>_ip; set=>this.RaiseAndSetIfChanged(ref _ip,value); }
    public string MacAddress { get=>_mac; set=>this.RaiseAndSetIfChanged(ref _mac,value); }
    public string ControllerMacAddress { get=>_controllerMac; private set=>this.RaiseAndSetIfChanged(ref _controllerMac,value); }
    public string CertificatePath { get=>_certificatePath; set=>this.RaiseAndSetIfChanged(ref _certificatePath,value); }
    public string DeviceUuid { get=>_uuid; set=>this.RaiseAndSetIfChanged(ref _uuid,value); }
    public string Status { get=>_status; private set=>this.RaiseAndSetIfChanged(ref _status,value); }
    public string Pin { get=>_pin; set=>this.RaiseAndSetIfChanged(ref _pin,value); }
    public string KeyMacro { get=>_keyMacro; set=>this.RaiseAndSetIfChanged(ref _keyMacro,value); }
    public string CapturedTraffic { get=>_capturedTraffic; private set=>this.RaiseAndSetIfChanged(ref _capturedTraffic,value); }
    public string PictureSettingsCatalog { get=>_pictureSettingsCatalog; private set=>this.RaiseAndSetIfChanged(ref _pictureSettingsCatalog,value); }
    public int PictureMenuId { get=>_pictureMenuId; set=>this.RaiseAndSetIfChanged(ref _pictureMenuId,value); }
    public int PictureMenuValue { get=>_pictureMenuValue; set=>this.RaiseAndSetIfChanged(ref _pictureMenuValue,value); }
    public string LaserAction { get=>_laserAction; set=>this.RaiseAndSetIfChanged(ref _laserAction,value); }
    public double LaserLevel
    {
        get=>_laserLevel;
        set
        {
            var normalized=Math.Clamp(Math.Round(value),0,10);
            if(Math.Abs(_laserLevel-normalized)<0.5)return;
            this.RaiseAndSetIfChanged(ref _laserLevel,normalized);
            QueueLaserSet((int)normalized);
        }
    }
    public double Volume
    {
        get=>_volume;
        set
        {
            var normalized=Math.Clamp(Math.Round(value),0,100);
            if(Math.Abs(_volume-normalized)<0.5)return;
            this.RaiseAndSetIfChanged(ref _volume,normalized);
            if(!_applyingVolume) QueueVolumeSet((int)normalized);
        }
    }
    public bool Listening { get=>_listening; private set { this.RaiseAndSetIfChanged(ref _listening,value); this.RaisePropertyChanged(nameof(TrafficButtonText)); } }
    public string TrafficButtonText => Listening ? "Stop listening" : "Start listening";
    public bool Paired { get=>_paired; private set=>this.RaiseAndSetIfChanged(ref _paired,value); }
    public bool ShowSetup { get=>_setup; set=>this.RaiseAndSetIfChanged(ref _setup,value); }
    public ICommand SaveCommand { get; }
    public ICommand PowerOnCommand { get; }
    public ICommand PowerOffCommand { get; }
    public ICommand KeyCommand { get; }
    public ICommand RequestPinCommand { get; }
    public ICommand PairCommand { get; }
    public ICommand RunMacroCommand { get; }
    public ICommand SetPictureSettingCommand { get; }
    public ICommand GetPictureSettingsCommand { get; }
    public ICommand SendLaserTestCommand { get; }
    public ICommand CaptureTrafficCommand { get; }
    public ICommand ClearTrafficCommand { get; }
    public ICommand ToggleSetupCommand { get; }
    public ICommand DiscoverCommand { get; }
    public HisenseControlViewModel(IHisenseVidaaService? service)
    {
        _service = service;
        SaveCommand = ReactiveCommand.CreateFromTask(SaveAsync);
        PowerOnCommand = ReactiveCommand.CreateFromTask(WakeAsync);
        PowerOffCommand = ReactiveCommand.CreateFromTask(() => KeyAsync("KEY_POWER"));
        KeyCommand = ReactiveCommand.CreateFromTask<string>(KeyAsync);
        RequestPinCommand = ReactiveCommand.CreateFromTask(RequestPinAsync);
        PairCommand = ReactiveCommand.CreateFromTask(PairAsync);
        RunMacroCommand = ReactiveCommand.CreateFromTask(RunMacroAsync);
        SetPictureSettingCommand = ReactiveCommand.CreateFromTask(SetPictureSettingAsync);
        GetPictureSettingsCommand = ReactiveCommand.CreateFromTask(GetPictureSettingsAsync);
        SendLaserTestCommand = ReactiveCommand.CreateFromTask(SendLaserTestAsync);
        CaptureTrafficCommand = ReactiveCommand.Create(ToggleTrafficListening);
        ClearTrafficCommand = ReactiveCommand.Create(ClearTraffic);
        ToggleSetupCommand = ReactiveCommand.Create(() => ShowSetup = !ShowSetup);
        DiscoverCommand = ReactiveCommand.CreateFromTask(DiscoverAsync);
    }
    public HisenseControlViewModel() : this(null) { }
    public void SetMonitor(PhysicalMonitor? monitor)
    {
        if (_monitor?.Id != monitor?.Id)
        {
            _trafficCancellation?.Cancel();
            _volumeSetCancellation?.Cancel();
            _volumeRefreshCancellation?.Cancel();
            _laserSetCancellation?.Cancel();
        }
        _monitor=monitor; if (monitor is null) { IsVisible=false; return; }
        var c=_service?.GetConfiguration(monitor.Id); IsVisible=monitor.Model.PnpCode.StartsWith("HEC",StringComparison.OrdinalIgnoreCase) || c is not null;
        if(c is not null){ IpAddress=c.IpAddress; MacAddress=c.MacAddress; ControllerMacAddress=c.ControllerMacAddress; CertificatePath=c.ClientCertificatePath; DeviceUuid=c.DeviceUuid; KeyMacro=c.KeyMacro; Paired=c.HasPairing; ShowSetup=!Paired; Status=Paired ? $"Connected to VIDAA. Certificate SHA-256: {DeviceCertificatePin.Display(c.ServerCertificateFingerprint)}" : "Address saved; secure pairing is required."; if(Paired) BeginVolumeRefresh(monitor.Id); }
        else { KeyMacro="KEY_BRIGHTNESSUP"; Paired=false; ShowSetup=true; Status="Enter the projector IP address; the controller MAC is detected automatically."; }
    }
    async Task SaveAsync(){ if(_service is null||_monitor is null)return; await Run(async ct=>{_service.SaveAddress(_monitor.Id,IpAddress,MacAddress,DeviceUuid,CertificatePath); var c=_service.GetConfiguration(_monitor.Id); ControllerMacAddress=c?.ControllerMacAddress ?? ""; CertificatePath=c?.ClientCertificatePath ?? CertificatePath; Status="Address saved; controller network interface detected automatically.";}); }
    async Task DiscoverAsync(){ if(_service is null)return; await Run(async ct=>{ Status="Searching the configured /24 for the VIDAA projector…"; var d=await _service.FindAsync(IpAddress,ct); IpAddress=d.IpAddress; if(_monitor is not null)_service.SaveAddress(_monitor.Id,IpAddress,MacAddress,DeviceUuid,CertificatePath); var c=_monitor is null?null:_service.GetConfiguration(_monitor.Id); ControllerMacAddress=c?.ControllerMacAddress??ControllerMacAddress; Status=$"Found {d.Name} at {d.IpAddress}, VIDAA protocol {d.ProtocolVersion?.ToString() ?? "unknown"}."; },TimeSpan.FromSeconds(20)); }
    async Task KeyAsync(string key){if(_service is null||_monitor is null)return;var monitorId=_monitor.Id;await Run(async ct=>{await _service.SendKeyAsync(monitorId,key,ct);Status=$"Sent {key}.";});if(key.Contains("VOL",StringComparison.OrdinalIgnoreCase)||key.Contains("MUTE",StringComparison.OrdinalIgnoreCase))BeginVolumeRefresh(monitorId);}
    async Task RequestPinAsync(){if(_service is null||_monitor is null)return;await Run(async ct=>{await _service.RequestPinAsync(_monitor.Id,ct);var c=_service.GetConfiguration(_monitor.Id);IpAddress=c?.IpAddress??IpAddress;ControllerMacAddress=c?.ControllerMacAddress??ControllerMacAddress;Status="Enter the VIDAA pairing PIN now shown on the projector.";},TimeSpan.FromSeconds(35));}
    async Task PairAsync(){if(_service is null||_monitor is null)return;var monitorId=_monitor.Id;await Run(async ct=>{await _service.PairAsync(monitorId,Pin,ct);var c=_service.GetConfiguration(monitorId);IpAddress=c?.IpAddress??IpAddress;Paired=c?.HasPairing??false;ShowSetup=!Paired;Status=Paired?$"Paired. Certificate SHA-256: {DeviceCertificatePin.Display(c?.ServerCertificateFingerprint)}":"Pairing did not produce a reusable secure connection.";},TimeSpan.FromSeconds(45));if(Paired)BeginVolumeRefresh(monitorId);}
    async Task RunMacroAsync(){if(_service is null||_monitor is null)return;await Run(async ct=>{var sequence=RemoteMacro.Parse(KeyMacro);_service.SaveKeyMacro(_monitor.Id,KeyMacro);await _service.SendSequenceAsync(_monitor.Id,sequence,ct);Status=$"Macro completed ({sequence.Count} keys).";},TimeSpan.FromSeconds(45));}
    async Task SetPictureSettingAsync(){if(_service is null||_monitor is null)return;await Run(async ct=>{await _service.SetPictureSettingAsync(_monitor.Id,PictureMenuId,PictureMenuValue,ct);Status=$"Sent picture setting menu {PictureMenuId} = {PictureMenuValue}.";});}
    async Task GetPictureSettingsAsync()
    {
        if(_service is null||_monitor is null)return;
        PictureSettingsCatalog="Requesting the VIDAA picture-setting catalogue…";
        await Run(async ct=>
        {
            var settings=await _service.GetPictureSettingsAsync(_monitor.Id,ct);
            PictureSettingsCatalog=string.Join("\n",settings
                .OrderBy(setting=>setting.MenuId)
                .Select(setting=>$"#{setting.MenuId} · {(string.IsNullOrWhiteSpace(setting.Name)?"unnamed":setting.Name)} = {setting.Value}"+
                                $"{(string.IsNullOrWhiteSpace(setting.ValueType)?"":$" [{setting.ValueType}]")}"+
                                $"{(setting.Flag is null?"":$" · flag {setting.Flag}")}"));
            var laser=settings.FirstOrDefault(setting=>
                setting.Name.Contains("laser",StringComparison.OrdinalIgnoreCase)
                && (setting.Name.Contains("lumin",StringComparison.OrdinalIgnoreCase)
                    || setting.Name.Contains("bright",StringComparison.OrdinalIgnoreCase)
                    || setting.Name.Contains("light",StringComparison.OrdinalIgnoreCase)));
            if(laser is not null)
            {
                PictureMenuId=laser.MenuId;
                if(int.TryParse(laser.Value,out var value))PictureMenuValue=value;
                Status=$"Found {laser.Name}: menu {laser.MenuId}, current value {laser.Value}. Selected it below.";
            }
            else
            {
                Status=$"VIDAA returned {settings.Count} picture setting{(settings.Count==1?"":"s")}; no named laser setting was found.";
            }
        });
    }
    async Task SendLaserTestAsync()
    {
        if(_service is null||_monitor is null)return;
        _laserSetCancellation?.Cancel();
        var monitorId=_monitor.Id;
        var action=LaserAction;
        var level=(int)LaserLevel;
        await Run(async ct=>
        {
            await _service.SendPlatformActionAsync(monitorId,action,level,ct);
            Status=$"Sent {action.Trim()} = {level} (unconfirmed).";
        });
    }
    void ToggleTrafficListening()
    {
        if (Listening)
        {
            Status="Stopping MQTT listener…";
            _trafficCancellation?.Cancel();
            return;
        }
        if (_service is null || _monitor is null) return;
        ClearTraffic();
        CapturedTraffic="Listening for MQTT messages…";
        Status="Listening continuously to VIDAA MQTT. Commands remain available.";
        var cancellation = new CancellationTokenSource();
        _trafficCancellation=cancellation;
        Listening=true;
        _ = ListenTrafficAsync(_monitor.Id,cancellation);
    }
    async Task ListenTrafficAsync(string monitorId, CancellationTokenSource cancellation)
    {
        string? failure=null;
        try { await _service!.ListenTrafficAsync(monitorId,AppendTraffic,cancellation.Token); }
        catch(OperationCanceledException) when(cancellation.IsCancellationRequested) { }
        catch(Exception e) { failure=e.Message; }
        finally
        {
            await Dispatcher.UIThread.InvokeAsync(() =>
            {
                if (!ReferenceEquals(_trafficCancellation,cancellation)) return;
                _trafficCancellation=null;
                Listening=false;
                Status=failure ?? "MQTT listener stopped.";
            });
            cancellation.Dispose();
        }
    }
    void AppendTraffic(VidaaTrafficMessage message) => Dispatcher.UIThread.Post(() =>
    {
        if(HisenseVidaaProtocol.TryParseVolume(message.Topic,message.Payload,out var volume))ApplyVolume(volume);
        _trafficMessages.Enqueue(message);
        while(_trafficMessages.Count>200) _trafficMessages.Dequeue();
        CapturedTraffic=VidaaTrafficCapture.Format(_trafficMessages.Reverse());
        Status=$"Listening to VIDAA MQTT · {_trafficMessages.Count} message{(_trafficMessages.Count==1?"":"s")}.";
    });
    void QueueVolumeSet(int volume)
    {
        if(_service is null||_monitor is null||!Paired)return;
        _volumeSetCancellation?.Cancel();
        var cancellation=new CancellationTokenSource();
        _volumeSetCancellation=cancellation;
        _=SetVolumeAfterDelayAsync(_monitor.Id,volume,cancellation);
    }
    void QueueLaserSet(int level)
    {
        if(_service is null||_monitor is null||!Paired)return;
        _laserSetCancellation?.Cancel();
        var cancellation=new CancellationTokenSource();
        _laserSetCancellation=cancellation;
        _=SetLaserAfterDelayAsync(_monitor.Id,LaserAction,level,cancellation);
    }
    async Task SetLaserAfterDelayAsync(string monitorId,string action,int level,CancellationTokenSource cancellation)
    {
        try
        {
            await Task.Delay(120,cancellation.Token);
            await _service!.SendPlatformActionAsync(monitorId,action,level,cancellation.Token);
            await Dispatcher.UIThread.InvokeAsync(()=>
            {
                if(_monitor?.Id==monitorId)Status=$"Sent {action.Trim()} = {level} (unconfirmed).";
            });
        }
        catch(OperationCanceledException) when(cancellation.IsCancellationRequested) { }
        catch(Exception e){await Dispatcher.UIThread.InvokeAsync(()=>Status=e.Message);}
        finally{if(ReferenceEquals(_laserSetCancellation,cancellation))_laserSetCancellation=null;cancellation.Dispose();}
    }
    async Task SetVolumeAfterDelayAsync(string monitorId,int volume,CancellationTokenSource cancellation)
    {
        try
        {
            await Task.Delay(120,cancellation.Token);
            var confirmed=await _service!.SetVolumeAsync(monitorId,volume,cancellation.Token);
            await Dispatcher.UIThread.InvokeAsync(()=>{if(_monitor?.Id==monitorId){ApplyVolume(confirmed);Status=$"Volume {confirmed}.";}});
        }
        catch(OperationCanceledException) when(cancellation.IsCancellationRequested) { }
        catch(Exception e){await Dispatcher.UIThread.InvokeAsync(()=>Status=e.Message);}
        finally{if(ReferenceEquals(_volumeSetCancellation,cancellation))_volumeSetCancellation=null;cancellation.Dispose();}
    }
    void BeginVolumeRefresh(string monitorId)
    {
        if(_service is null)return;
        _volumeRefreshCancellation?.Cancel();
        var cancellation=new CancellationTokenSource(TimeSpan.FromSeconds(8));
        _volumeRefreshCancellation=cancellation;
        _=RefreshVolumeAsync(monitorId,cancellation);
    }
    async Task RefreshVolumeAsync(string monitorId,CancellationTokenSource cancellation)
    {
        try
        {
            var volume=await _service!.GetVolumeAsync(monitorId,cancellation.Token);
            await Dispatcher.UIThread.InvokeAsync(()=>{if(_monitor?.Id==monitorId)ApplyVolume(volume);});
        }
        catch(OperationCanceledException) { }
        catch(Exception) { /* The slider remains usable even if the initial query is unsupported. */ }
        finally{if(ReferenceEquals(_volumeRefreshCancellation,cancellation))_volumeRefreshCancellation=null;cancellation.Dispose();}
    }
    void ApplyVolume(int volume){_applyingVolume=true;try{Volume=volume;}finally{_applyingVolume=false;}}
    void ClearTraffic(){_trafficMessages.Clear();CapturedTraffic=Listening?"Listening for MQTT messages…":"";}
    async Task WakeAsync(){if(_service is null||_monitor is null)return;await Run(async ct=>{_service.SaveAddress(_monitor.Id,IpAddress,MacAddress,DeviceUuid,CertificatePath);await _service.PowerOnAsync(_monitor.Id,ct);Status="Wake-on-LAN sent.";});}
    async Task Run(Func<CancellationToken,Task> action,TimeSpan? timeout=null){if(Busy)return;Busy=true;using var c=new CancellationTokenSource(timeout??TimeSpan.FromSeconds(15));try{await action(c.Token);}catch(OperationCanceledException){Status="Timed out while contacting the projector. Check that it is awake and still reachable at the configured IP address.";}catch(Exception e){Status=e.Message;}finally{Busy=false;}}
    public void Dispose(){_trafficCancellation?.Cancel();_volumeSetCancellation?.Cancel();_volumeRefreshCancellation?.Cancel();_laserSetCancellation?.Cancel();}
}
