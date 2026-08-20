#nullable enable
using System.Windows.Input;
using Avalonia.Threading;
using LittleBigMouse.DisplayLayout.Monitors;
using ReactiveUI;

namespace LittleBigMouse.Plugin.Vcp.Avalonia.HisenseVidaa;

public sealed class HisenseControlViewModel : ReactiveObject, IDisposable
{
    readonly IHisenseVidaaService? _service;
    readonly ExclusiveOperationRunner _operations;
    readonly Queue<VidaaTrafficMessage> _trafficMessages = new();
    CancellationTokenSource? _trafficCancellation;
    CancellationTokenSource? _volumeSetCancellation;
    CancellationTokenSource? _volumeRefreshCancellation;
    CancellationTokenSource? _laserSetCancellation;
    PhysicalMonitor? _monitor;
    string _ip = "";
    string _mac = "";
    string _controllerMac = "";
    string _certificatePath = "";
    string _uuid = "";
    string _pin = "";
    string _keyMacro = "KEY_BRIGHTNESSUP";
    string _capturedTraffic = "";
    string _pictureSettingsCatalog = "";
    string _status = "";
    string _laserAction = "changelaserluminance";
    int _pictureMenuId = 2;
    int _pictureMenuValue = 5;
    double _volume;
    double _laserLevel = 5;
    bool _visible;
    bool _busy;
    bool _paired;
    bool _setup;
    bool _listening;
    bool _applyingVolume;

    public bool IsVisible
    {
        get => _visible;
        private set => this.RaiseAndSetIfChanged(ref _visible, value);
    }

    public bool Busy
    {
        get => _busy;
        private set => this.RaiseAndSetIfChanged(ref _busy, value);
    }

    public string IpAddress
    {
        get => _ip;
        set => this.RaiseAndSetIfChanged(ref _ip, value);
    }

    public string MacAddress
    {
        get => _mac;
        set => this.RaiseAndSetIfChanged(ref _mac, value);
    }

    public string ControllerMacAddress
    {
        get => _controllerMac;
        private set => this.RaiseAndSetIfChanged(ref _controllerMac, value);
    }

    public string CertificatePath
    {
        get => _certificatePath;
        set => this.RaiseAndSetIfChanged(ref _certificatePath, value);
    }

    public string DeviceUuid
    {
        get => _uuid;
        set => this.RaiseAndSetIfChanged(ref _uuid, value);
    }

    public string Status
    {
        get => _status;
        private set => this.RaiseAndSetIfChanged(ref _status, value);
    }

    public string Pin
    {
        get => _pin;
        set => this.RaiseAndSetIfChanged(ref _pin, value);
    }

    public string KeyMacro
    {
        get => _keyMacro;
        set => this.RaiseAndSetIfChanged(ref _keyMacro, value);
    }

    public string CapturedTraffic
    {
        get => _capturedTraffic;
        private set => this.RaiseAndSetIfChanged(ref _capturedTraffic, value);
    }

    public string PictureSettingsCatalog
    {
        get => _pictureSettingsCatalog;
        private set => this.RaiseAndSetIfChanged(ref _pictureSettingsCatalog, value);
    }

    public int PictureMenuId
    {
        get => _pictureMenuId;
        set => this.RaiseAndSetIfChanged(ref _pictureMenuId, value);
    }

    public int PictureMenuValue
    {
        get => _pictureMenuValue;
        set => this.RaiseAndSetIfChanged(ref _pictureMenuValue, value);
    }

    public string LaserAction
    {
        get => _laserAction;
        set => this.RaiseAndSetIfChanged(ref _laserAction, value);
    }

    public double LaserLevel
    {
        get => _laserLevel;
        set
        {
            var normalizedLevel = Math.Clamp(Math.Round(value), 0, 10);
            var levelIsUnchanged = Math.Abs(_laserLevel - normalizedLevel) < 0.5;
            if (levelIsUnchanged)
            {
                return;
            }

            this.RaiseAndSetIfChanged(ref _laserLevel, normalizedLevel);
            QueueLaserSet((int)normalizedLevel);
        }
    }

    public double Volume
    {
        get => _volume;
        set
        {
            var normalizedVolume = Math.Clamp(Math.Round(value), 0, 100);
            var volumeIsUnchanged = Math.Abs(_volume - normalizedVolume) < 0.5;
            if (volumeIsUnchanged)
            {
                return;
            }

            this.RaiseAndSetIfChanged(ref _volume, normalizedVolume);
            if (!_applyingVolume)
            {
                QueueVolumeSet((int)normalizedVolume);
            }
        }
    }

    public bool Listening
    {
        get => _listening;
        private set
        {
            this.RaiseAndSetIfChanged(ref _listening, value);
            this.RaisePropertyChanged(nameof(TrafficButtonText));
        }
    }

    public string TrafficButtonText => Listening ? "Stop listening" : "Start listening";

    public bool Paired
    {
        get => _paired;
        private set => this.RaiseAndSetIfChanged(ref _paired, value);
    }

    public bool ShowSetup
    {
        get => _setup;
        set => this.RaiseAndSetIfChanged(ref _setup, value);
    }

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
        _operations = new ExclusiveOperationRunner(
            busy => Busy = busy,
            status => Status = status,
            "Timed out while contacting the projector. Check that it is awake and still reachable at the configured IP address.",
            TimeSpan.FromSeconds(15));
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

    public HisenseControlViewModel() : this(null)
    {
    }

    public void SetMonitor(PhysicalMonitor? monitor)
    {
        var monitorChanged = _monitor?.Id != monitor?.Id;
        if (monitorChanged)
        {
            _trafficCancellation?.Cancel();
            _volumeSetCancellation?.Cancel();
            _volumeRefreshCancellation?.Cancel();
            _laserSetCancellation?.Cancel();
        }

        _monitor = monitor;
        if (monitor is null)
        {
            IsVisible = false;
            return;
        }

        var configuration = _service?.GetConfiguration(monitor.Id);
        var hasHisensePnpCode = monitor.Model.PnpCode.StartsWith("HEC", StringComparison.OrdinalIgnoreCase);
        var hasSavedConfiguration = configuration is not null;
        IsVisible = hasHisensePnpCode || hasSavedConfiguration;

        if (configuration is not null)
        {
            IpAddress = configuration.IpAddress;
            MacAddress = configuration.MacAddress;
            ControllerMacAddress = configuration.ControllerMacAddress;
            CertificatePath = configuration.ClientCertificatePath;
            DeviceUuid = configuration.DeviceUuid;
            KeyMacro = configuration.KeyMacro;
            Paired = configuration.HasPairing;
            ShowSetup = !Paired;
            Status = Paired
                ? "Connected to VIDAA."
                : "Address saved; request a PIN to pair.";

            if (Paired)
            {
                BeginVolumeRefresh(monitor.Id);
            }
        }
        else
        {
            KeyMacro = "KEY_BRIGHTNESSUP";
            Paired = false;
            ShowSetup = true;
            Status = "Enter the projector IP address; the controller MAC is detected automatically.";
        }
    }

    async Task SaveAsync()
    {
        if (_service is null || _monitor is null)
        {
            return;
        }

        await _operations.RunAsync(async cancellationToken =>
        {
            _service.SaveAddress(_monitor.Id, IpAddress, MacAddress, DeviceUuid, CertificatePath);
            var configuration = _service.GetConfiguration(_monitor.Id);
            ControllerMacAddress = configuration?.ControllerMacAddress ?? "";
            CertificatePath = configuration?.ClientCertificatePath ?? CertificatePath;
            Status = "Address saved; controller network interface detected automatically.";
        });
    }

    async Task DiscoverAsync()
    {
        if (_service is null)
        {
            return;
        }

        await _operations.RunAsync(async cancellationToken =>
        {
            Status = "Searching the configured /24 for the VIDAA projector…";
            var discoveredDevice = await _service.FindAsync(IpAddress, cancellationToken);
            IpAddress = discoveredDevice.IpAddress;

            if (_monitor is not null)
            {
                _service.SaveAddress(_monitor.Id, IpAddress, MacAddress, DeviceUuid, CertificatePath);
            }

            var configuration = _monitor is null
                ? null
                : _service.GetConfiguration(_monitor.Id);
            ControllerMacAddress = configuration?.ControllerMacAddress ?? ControllerMacAddress;
            var protocolVersion = discoveredDevice.ProtocolVersion?.ToString() ?? "unknown";
            Status = $"Found {discoveredDevice.Name} at {discoveredDevice.IpAddress}, VIDAA protocol {protocolVersion}.";
        }, TimeSpan.FromSeconds(20));
    }

    async Task KeyAsync(string key)
    {
        if (_service is null || _monitor is null)
        {
            return;
        }

        var monitorId = _monitor.Id;
        await _operations.RunAsync(async cancellationToken =>
        {
            await _service.SendKeyAsync(monitorId, key, cancellationToken);
            Status = $"Sent {key}.";
        });

        var changesVolume = key.Contains("VOL", StringComparison.OrdinalIgnoreCase);
        var changesMute = !changesVolume
            && key.Contains("MUTE", StringComparison.OrdinalIgnoreCase);
        if (changesVolume || changesMute)
        {
            BeginVolumeRefresh(monitorId);
        }
    }

    async Task RequestPinAsync()
    {
        if (_service is null || _monitor is null)
        {
            return;
        }

        await _operations.RunAsync(async cancellationToken =>
        {
            await _service.RequestPinAsync(_monitor.Id, cancellationToken);
            var configuration = _service.GetConfiguration(_monitor.Id);
            IpAddress = configuration?.IpAddress ?? IpAddress;
            ControllerMacAddress = configuration?.ControllerMacAddress ?? ControllerMacAddress;
            Status = "Enter the VIDAA pairing PIN now shown on the projector.";
        }, TimeSpan.FromSeconds(35));
    }

    async Task PairAsync()
    {
        if (_service is null || _monitor is null)
        {
            return;
        }

        var monitorId = _monitor.Id;
        await _operations.RunAsync(async cancellationToken =>
        {
            await _service.PairAsync(monitorId, Pin, cancellationToken);
            var configuration = _service.GetConfiguration(monitorId);
            IpAddress = configuration?.IpAddress ?? IpAddress;
            Paired = true;
            ShowSetup = false;
            Status = "Paired and connected over the local network.";
        }, TimeSpan.FromSeconds(45));

        if (Paired)
        {
            BeginVolumeRefresh(monitorId);
        }
    }

    async Task RunMacroAsync()
    {
        if (_service is null || _monitor is null)
        {
            return;
        }

        await _operations.RunAsync(async cancellationToken =>
        {
            var keySequence = RemoteMacro.Parse(KeyMacro);
            _service.SaveKeyMacro(_monitor.Id, KeyMacro);
            await _service.SendSequenceAsync(_monitor.Id, keySequence, cancellationToken);
            Status = $"Macro completed ({keySequence.Count} keys).";
        }, TimeSpan.FromSeconds(45));
    }

    async Task SetPictureSettingAsync()
    {
        if (_service is null || _monitor is null)
        {
            return;
        }

        await _operations.RunAsync(async cancellationToken =>
        {
            await _service.SetPictureSettingAsync(
                _monitor.Id,
                PictureMenuId,
                PictureMenuValue,
                cancellationToken);
            Status = $"Sent picture setting menu {PictureMenuId} = {PictureMenuValue}.";
        });
    }

    async Task GetPictureSettingsAsync()
    {
        if (_service is null || _monitor is null)
        {
            return;
        }

        PictureSettingsCatalog = "Requesting the VIDAA picture-setting catalogue…";
        await _operations.RunAsync(async cancellationToken =>
        {
            var settings = await _service.GetPictureSettingsAsync(_monitor.Id, cancellationToken);
            PictureSettingsCatalog = string.Join("\n", settings
                .OrderBy(setting => setting.MenuId)
                .Select(setting =>
                {
                    var menuId = setting.MenuId;
                    var settingName = string.IsNullOrWhiteSpace(setting.Name)
                        ? "unnamed"
                        : setting.Name;
                    var settingValue = setting.Value;
                    var valueTypeSuffix = string.IsNullOrWhiteSpace(setting.ValueType)
                        ? ""
                        : $" [{setting.ValueType}]";
                    var flagSuffix = setting.Flag is null
                        ? ""
                        : $" · flag {setting.Flag}";
                    return $"#{menuId} · {settingName} = {settingValue}{valueTypeSuffix}{flagSuffix}";
                }));

            var laserSetting = settings.FirstOrDefault(setting =>
            {
                var nameContainsLaser = setting.Name.Contains("laser", StringComparison.OrdinalIgnoreCase);
                if (!nameContainsLaser)
                {
                    return false;
                }

                var nameContainsLuminance = setting.Name.Contains("lumin", StringComparison.OrdinalIgnoreCase);
                if (nameContainsLuminance)
                {
                    return true;
                }

                var nameContainsBrightness = setting.Name.Contains("bright", StringComparison.OrdinalIgnoreCase);
                if (nameContainsBrightness)
                {
                    return true;
                }

                var nameContainsLight = setting.Name.Contains("light", StringComparison.OrdinalIgnoreCase);
                return nameContainsLight;
            });

            if (laserSetting is not null)
            {
                PictureMenuId = laserSetting.MenuId;
                var hasNumericValue = int.TryParse(laserSetting.Value, out var menuValue);
                if (hasNumericValue)
                {
                    PictureMenuValue = menuValue;
                }

                Status = $"Found {laserSetting.Name}: menu {laserSetting.MenuId}, current value {laserSetting.Value}. Selected it below.";
            }
            else
            {
                var settingCountSuffix = settings.Count == 1 ? "" : "s";
                Status = $"VIDAA returned {settings.Count} picture setting{settingCountSuffix}; no named laser setting was found.";
            }
        });
    }

    async Task SendLaserTestAsync()
    {
        if (_service is null || _monitor is null)
        {
            return;
        }

        _laserSetCancellation?.Cancel();
        var monitorId = _monitor.Id;
        var action = LaserAction;
        var level = (int)LaserLevel;
        await _operations.RunAsync(async cancellationToken =>
        {
            await _service.SendPlatformActionAsync(monitorId, action, level, cancellationToken);
            Status = $"Sent {action.Trim()} = {level} (unconfirmed).";
        });
    }

    void ToggleTrafficListening()
    {
        if (Listening)
        {
            Status = "Stopping MQTT listener…";
            _trafficCancellation?.Cancel();
            return;
        }

        if (_service is null || _monitor is null)
        {
            return;
        }

        ClearTraffic();
        CapturedTraffic = "Listening for MQTT messages…";
        Status = "Listening continuously to VIDAA MQTT. Commands remain available.";
        var cancellation = new CancellationTokenSource();
        _trafficCancellation = cancellation;
        Listening = true;
        _ = ListenTrafficAsync(_monitor.Id, cancellation);
    }

    async Task ListenTrafficAsync(string monitorId, CancellationTokenSource cancellation)
    {
        string? failure = null;
        try
        {
            await _service!.ListenTrafficAsync(monitorId, AppendTraffic, cancellation.Token);
        }
        catch (OperationCanceledException) when (cancellation.IsCancellationRequested)
        {
        }
        catch (Exception exception)
        {
            failure = exception.Message;
        }
        finally
        {
            await Dispatcher.UIThread.InvokeAsync(() =>
            {
                var isCurrentListener = ReferenceEquals(_trafficCancellation, cancellation);
                if (!isCurrentListener)
                {
                    return;
                }

                _trafficCancellation = null;
                Listening = false;
                Status = failure ?? "MQTT listener stopped.";
            });
            cancellation.Dispose();
        }
    }

    void AppendTraffic(VidaaTrafficMessage message) => Dispatcher.UIThread.Post(() =>
    {
        var containsVolume = HisenseVidaaProtocol.TryParseVolume(
            message.Topic,
            message.Payload,
            out var volume);
        if (containsVolume)
        {
            ApplyVolume(volume);
        }

        _trafficMessages.Enqueue(message);
        while (_trafficMessages.Count > 200)
        {
            _trafficMessages.Dequeue();
        }

        CapturedTraffic = VidaaTrafficCapture.Format(_trafficMessages.Reverse());
        var messageCountSuffix = _trafficMessages.Count == 1 ? "" : "s";
        Status = $"Listening to VIDAA MQTT · {_trafficMessages.Count} message{messageCountSuffix}.";
    });

    void QueueVolumeSet(int volume)
    {
        if (_service is null || _monitor is null || !Paired)
        {
            return;
        }

        _volumeSetCancellation?.Cancel();
        var cancellation = new CancellationTokenSource();
        _volumeSetCancellation = cancellation;
        _ = SetVolumeAfterDelayAsync(_monitor.Id, volume, cancellation);
    }

    void QueueLaserSet(int level)
    {
        if (_service is null || _monitor is null || !Paired)
        {
            return;
        }

        _laserSetCancellation?.Cancel();
        var cancellation = new CancellationTokenSource();
        _laserSetCancellation = cancellation;
        _ = SetLaserAfterDelayAsync(_monitor.Id, LaserAction, level, cancellation);
    }

    async Task SetLaserAfterDelayAsync(
        string monitorId,
        string action,
        int level,
        CancellationTokenSource cancellation)
    {
        try
        {
            await Task.Delay(120, cancellation.Token);
            await _service!.SendPlatformActionAsync(monitorId, action, level, cancellation.Token);
            await Dispatcher.UIThread.InvokeAsync(() =>
            {
                var monitorIsCurrent = _monitor?.Id == monitorId;
                if (monitorIsCurrent)
                {
                    Status = $"Sent {action.Trim()} = {level} (unconfirmed).";
                }
            });
        }
        catch (OperationCanceledException) when (cancellation.IsCancellationRequested)
        {
        }
        catch (Exception exception)
        {
            await Dispatcher.UIThread.InvokeAsync(() => Status = exception.Message);
        }
        finally
        {
            if (ReferenceEquals(_laserSetCancellation, cancellation))
            {
                _laserSetCancellation = null;
            }

            cancellation.Dispose();
        }
    }

    async Task SetVolumeAfterDelayAsync(
        string monitorId,
        int volume,
        CancellationTokenSource cancellation)
    {
        try
        {
            await Task.Delay(120, cancellation.Token);
            var confirmedVolume = await _service!.SetVolumeAsync(
                monitorId,
                volume,
                cancellation.Token);
            await Dispatcher.UIThread.InvokeAsync(() =>
            {
                var monitorIsCurrent = _monitor?.Id == monitorId;
                if (monitorIsCurrent)
                {
                    ApplyVolume(confirmedVolume);
                    Status = $"Volume {confirmedVolume}.";
                }
            });
        }
        catch (OperationCanceledException) when (cancellation.IsCancellationRequested)
        {
        }
        catch (Exception exception)
        {
            await Dispatcher.UIThread.InvokeAsync(() => Status = exception.Message);
        }
        finally
        {
            if (ReferenceEquals(_volumeSetCancellation, cancellation))
            {
                _volumeSetCancellation = null;
            }

            cancellation.Dispose();
        }
    }

    void BeginVolumeRefresh(string monitorId)
    {
        if (_service is null)
        {
            return;
        }

        _volumeRefreshCancellation?.Cancel();
        var cancellation = new CancellationTokenSource(TimeSpan.FromSeconds(8));
        _volumeRefreshCancellation = cancellation;
        _ = RefreshVolumeAsync(monitorId, cancellation);
    }

    async Task RefreshVolumeAsync(string monitorId, CancellationTokenSource cancellation)
    {
        try
        {
            var volume = await _service!.GetVolumeAsync(monitorId, cancellation.Token);
            await Dispatcher.UIThread.InvokeAsync(() =>
            {
                var monitorIsCurrent = _monitor?.Id == monitorId;
                if (monitorIsCurrent)
                {
                    ApplyVolume(volume);
                }
            });
        }
        catch (OperationCanceledException)
        {
        }
        catch (Exception)
        {
            // The slider remains usable even if the initial query is unsupported.
        }
        finally
        {
            if (ReferenceEquals(_volumeRefreshCancellation, cancellation))
            {
                _volumeRefreshCancellation = null;
            }

            cancellation.Dispose();
        }
    }

    void ApplyVolume(int volume)
    {
        _applyingVolume = true;
        try
        {
            Volume = volume;
        }
        finally
        {
            _applyingVolume = false;
        }
    }

    void ClearTraffic()
    {
        _trafficMessages.Clear();
        CapturedTraffic = Listening ? "Listening for MQTT messages…" : "";
    }

    async Task WakeAsync()
    {
        if (_service is null || _monitor is null)
        {
            return;
        }

        await _operations.RunAsync(async cancellationToken =>
        {
            _service.SaveAddress(_monitor.Id, IpAddress, MacAddress, DeviceUuid, CertificatePath);
            await _service.PowerOnAsync(_monitor.Id, cancellationToken);
            Status = "Wake-on-LAN sent.";
        });
    }

    public void Dispose()
    {
        _trafficCancellation?.Cancel();
        _volumeSetCancellation?.Cancel();
        _volumeRefreshCancellation?.Cancel();
        _laserSetCancellation?.Cancel();
    }
}
