#nullable enable
using System.Collections.ObjectModel;
using System.Windows.Input;
using LittleBigMouse.DisplayLayout.Monitors;
using ReactiveUI;

namespace LittleBigMouse.Plugin.Vcp.Avalonia.SamsungTizen;

public sealed class SamsungControlViewModel : ReactiveObject
{
    readonly ISamsungTizenService? _service;
    PhysicalMonitor? _monitor;

    public SamsungControlViewModel(ISamsungTizenService? service)
    {
        _service = service;
        DiscoverCommand = ReactiveCommand.CreateFromTask(DiscoverAsync);
        PairCommand = ReactiveCommand.CreateFromTask(PairAsync);
        PowerOnCommand = ReactiveCommand.CreateFromTask(PowerOnAsync);
        PowerOffCommand = ReactiveCommand.CreateFromTask(() => SendKeyAsync("KEY_POWEROFF", "Power off sent."));
        RemoteKeyCommand = ReactiveCommand.CreateFromTask<string>(key => SendKeyAsync(key, $"Sent {key}."));
        RunMacroCommand = ReactiveCommand.CreateFromTask(RunMacroAsync);
        ToggleSetupCommand = ReactiveCommand.Create(() => ShowSetup = !ShowSetup);
    }

    public void SetMonitor(PhysicalMonitor? monitor)
    {
        _monitor = monitor;
        DiscoveredDevices.Clear();
        HasDiscoveredDevices = false;
        SelectedDevice = null;

        if (monitor is null)
        {
            IsVisible = false;
            ShowSetup = false;
            return;
        }

        var configuration = _service?.GetConfiguration(monitor.Id);
        var samsungEdid = monitor.Model.PnpCode.StartsWith("SAM", StringComparison.OrdinalIgnoreCase);
        IsVisible = samsungEdid || configuration is not null;

        if (configuration is null)
        {
            IpAddress = "";
            MacAddress = "";
            DeviceName = "Samsung Tizen";
            PictureMacro = "";
            Status = "Enter the monitor IP address or run discovery.";
            Paired = false;
            ShowSetup = true;
            return;
        }

        Apply(configuration);
        Status = string.IsNullOrWhiteSpace(configuration.Token)
                 || string.IsNullOrWhiteSpace(configuration.ServerCertificateFingerprint)
            ? "Address saved; secure pairing is required."
            : $"Associated with this monitor. Certificate SHA-256: {DeviceCertificatePin.Display(configuration.ServerCertificateFingerprint)}";
    }

    public ObservableCollection<SamsungTizenDevice> DiscoveredDevices { get; } = [];

    public bool HasDiscoveredDevices
    {
        get => _hasDiscoveredDevices;
        private set => this.RaiseAndSetIfChanged(ref _hasDiscoveredDevices, value);
    }
    bool _hasDiscoveredDevices;

    public SamsungTizenDevice? SelectedDevice
    {
        get => _selectedDevice;
        set
        {
            this.RaiseAndSetIfChanged(ref _selectedDevice, value);
            if (value is null) return;
            IpAddress = value.IpAddress;
            if (!string.IsNullOrWhiteSpace(value.MacAddress)) MacAddress = value.MacAddress;
            DeviceName = string.IsNullOrWhiteSpace(value.ModelName)
                ? value.Name
                : $"{value.Name} · {value.ModelName}";
        }
    }
    SamsungTizenDevice? _selectedDevice;

    public string IpAddress
    {
        get => _ipAddress;
        set => this.RaiseAndSetIfChanged(ref _ipAddress, value);
    }
    string _ipAddress = "";

    public string MacAddress
    {
        get => _macAddress;
        set => this.RaiseAndSetIfChanged(ref _macAddress, value);
    }
    string _macAddress = "";

    public string PictureMacro
    {
        get => _pictureMacro;
        set => this.RaiseAndSetIfChanged(ref _pictureMacro, value);
    }
    string _pictureMacro = "";

    public string DeviceName
    {
        get => _deviceName;
        private set => this.RaiseAndSetIfChanged(ref _deviceName, value);
    }
    string _deviceName = "Samsung Tizen";

    public string Status
    {
        get => _status;
        private set => this.RaiseAndSetIfChanged(ref _status, value);
    }
    string _status = "";

    public bool IsVisible
    {
        get => _isVisible;
        private set => this.RaiseAndSetIfChanged(ref _isVisible, value);
    }
    bool _isVisible;

    public bool Busy
    {
        get => _busy;
        private set => this.RaiseAndSetIfChanged(ref _busy, value);
    }
    bool _busy;

    public bool Paired
    {
        get => _paired;
        private set => this.RaiseAndSetIfChanged(ref _paired, value);
    }
    bool _paired;

    public bool ShowSetup
    {
        get => _showSetup;
        private set => this.RaiseAndSetIfChanged(ref _showSetup, value);
    }
    bool _showSetup;

    public ICommand DiscoverCommand { get; }
    public ICommand PairCommand { get; }
    public ICommand PowerOnCommand { get; }
    public ICommand PowerOffCommand { get; }
    public ICommand RemoteKeyCommand { get; }
    public ICommand RunMacroCommand { get; }
    public ICommand ToggleSetupCommand { get; }

    async Task DiscoverAsync()
    {
        if (_service is null) return;
        await RunAsync(async cancellationToken =>
        {
            Status = "Searching the local network…";
            var devices = await _service.DiscoverAsync(cancellationToken);
            DiscoveredDevices.Clear();
            foreach (var device in devices) DiscoveredDevices.Add(device);
            HasDiscoveredDevices = devices.Count > 0;
            SelectedDevice = devices.Count == 1 ? devices[0] : null;
            Status = devices.Count switch
            {
                0 => "No Samsung Tizen display found; enter its IP address.",
                1 => "Display found. Select Pair to authorize LittleBigMouse.",
                _ => $"{devices.Count} Samsung displays found; select one.",
            };
        }, TimeSpan.FromSeconds(8));
    }

    async Task PairAsync()
    {
        if (_service is null || _monitor is null) return;
        await RunAsync(async cancellationToken =>
        {
            Status = "Allow LittleBigMouse on the Samsung display…";
            var configuration = await _service.PairAsync(
                _monitor.Id, IpAddress, MacAddress, cancellationToken);
            Apply(configuration);
            Status = $"Paired. Certificate SHA-256: {DeviceCertificatePin.Display(configuration.ServerCertificateFingerprint)}";
        }, TimeSpan.FromSeconds(90));
    }

    async Task PowerOnAsync()
    {
        if (_service is null || _monitor is null) return;
        await RunAsync(async cancellationToken =>
        {
            _service.SaveManualAddress(_monitor.Id, IpAddress, MacAddress);
            await _service.PowerOnAsync(_monitor.Id, cancellationToken);
            Status = "Wake-on-LAN sent.";
        }, TimeSpan.FromSeconds(5));
    }

    async Task SendKeyAsync(string key, string success)
    {
        if (_service is null || _monitor is null) return;
        await RunAsync(async cancellationToken =>
        {
            await _service.SendKeyAsync(_monitor.Id, key, cancellationToken);
            Paired = true;
            ShowSetup = false;
            Status = success;
        }, TimeSpan.FromSeconds(15));
    }

    async Task RunMacroAsync()
    {
        if (_service is null || _monitor is null) return;
        await RunAsync(async cancellationToken =>
        {
            var sequence = SamsungTizenProtocol.ParseSequence(PictureMacro);
            _service.SavePictureMacro(_monitor.Id, PictureMacro);
            await _service.SendSequenceAsync(_monitor.Id, sequence, cancellationToken);
            Status = $"Macro completed ({sequence.Count} keys).";
        }, TimeSpan.FromSeconds(45));
    }

    async Task RunAsync(Func<CancellationToken, Task> action, TimeSpan timeout)
    {
        if (Busy) return;
        Busy = true;
        using var cancellation = new CancellationTokenSource(timeout);
        try
        {
            await action(cancellation.Token);
        }
        catch (OperationCanceledException)
        {
            Status = "The Samsung display did not answer in time.";
        }
        catch (Exception e)
        {
            Status = e.Message;
        }
        finally
        {
            Busy = false;
        }
    }

    void Apply(SamsungTizenConfiguration configuration)
    {
        IpAddress = configuration.IpAddress;
        MacAddress = configuration.MacAddress;
        DeviceName = string.IsNullOrWhiteSpace(configuration.ModelName)
            ? configuration.Name
            : $"{configuration.Name} · {configuration.ModelName}";
        PictureMacro = configuration.PictureMacro;
        Paired = !string.IsNullOrWhiteSpace(configuration.Token)
                 && !string.IsNullOrWhiteSpace(configuration.ServerCertificateFingerprint);
        ShowSetup = !Paired;
    }
}
