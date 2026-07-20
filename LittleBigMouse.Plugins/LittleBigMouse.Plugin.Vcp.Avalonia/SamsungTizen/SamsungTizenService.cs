#nullable enable
using System.Net.Http;

namespace LittleBigMouse.Plugin.Vcp.Avalonia.SamsungTizen;

public sealed class SamsungTizenService : ISamsungTizenService, IAsyncDisposable
{
    readonly SamsungTizenSettingsStore _store;
    readonly SamsungTizenDiscovery _discovery;
    readonly HttpClient _httpClient;
    readonly object _lock = new();
    readonly Dictionary<string, SamsungTizenClient> _clients = [];

    public SamsungTizenService(SamsungTizenSettingsStore store)
    {
        _store = store;
        _httpClient = new HttpClient { Timeout = TimeSpan.FromSeconds(4) };
        _discovery = new SamsungTizenDiscovery(_httpClient);
    }

    public SamsungTizenConfiguration? GetConfiguration(string monitorId) => _store.Get(monitorId);

    public Task<IReadOnlyList<SamsungTizenDevice>> DiscoverAsync(CancellationToken cancellationToken = default)
        => _discovery.DiscoverAsync(TimeSpan.FromSeconds(3), cancellationToken);

    public Task<SamsungTizenDevice> ProbeAsync(string ipAddress, CancellationToken cancellationToken = default)
        => _discovery.ProbeAsync(ipAddress.Trim(), cancellationToken);

    public async Task<SamsungTizenConfiguration> PairAsync(
        string monitorId,
        string ipAddress,
        string? macAddress = null,
        CancellationToken cancellationToken = default)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(monitorId);
        var device = await ProbeAsync(ipAddress, cancellationToken).ConfigureAwait(false);
        var existing = _store.Get(monitorId);
        var configuration = new SamsungTizenConfiguration
        {
            MonitorId = monitorId,
            IpAddress = device.IpAddress,
            MacAddress = !string.IsNullOrWhiteSpace(device.MacAddress)
                ? device.MacAddress
                : macAddress?.Trim() ?? existing?.MacAddress ?? "",
            // Explicit Pair always requests a fresh on-device authorization before
            // accepting a new certificate, so an old token is never exposed to a
            // replacement endpoint.
            Token = "",
            ServerCertificateFingerprint = existing?.IpAddress == device.IpAddress
                ? existing.ServerCertificateFingerprint : "",
            DeviceId = device.DeviceId,
            Name = device.Name,
            ModelName = device.ModelName,
            PictureMacro = existing?.PictureMacro ?? "",
        };

        var client = new SamsungTizenClient(configuration.IpAddress,
            expectedCertificateFingerprint: configuration.ServerCertificateFingerprint,
            allowNewCertificate: true);
        try
        {
            configuration.Token = await client.ConnectAsync(cancellationToken).ConfigureAwait(false);
            configuration.ServerCertificateFingerprint = client.ServerCertificateFingerprint;
        }
        catch
        {
            await client.DisposeAsync().ConfigureAwait(false);
            throw;
        }

        SamsungTizenClient? previous;
        lock (_lock)
        {
            _clients.Remove(monitorId, out previous);
            _clients[monitorId] = client;
        }
        if (previous is not null) await previous.DisposeAsync().ConfigureAwait(false);

        _store.Save(configuration);
        return configuration;
    }

    public void SaveManualAddress(string monitorId, string ipAddress, string macAddress)
    {
        if (!SamsungTizenDevice.IsValidAddress(ipAddress))
            throw new ArgumentException("Enter a valid IPv4 address.", nameof(ipAddress));

        var configuration = _store.Get(monitorId) ?? new SamsungTizenConfiguration { MonitorId = monitorId };
        if (!configuration.IpAddress.Equals(ipAddress.Trim(), StringComparison.Ordinal))
        {
            configuration.Token = "";
            configuration.ServerCertificateFingerprint = "";
        }
        configuration.IpAddress = ipAddress.Trim();
        configuration.MacAddress = macAddress.Trim();
        _store.Save(configuration);
    }

    public void SavePictureMacro(string monitorId, string sequence)
    {
        // Validate before persisting so a broken preset is never silently accepted.
        SamsungTizenProtocol.ParseSequence(sequence);
        var configuration = RequiredConfiguration(monitorId);
        configuration.PictureMacro = sequence.Trim();
        _store.Save(configuration);
    }

    public async Task SendKeyAsync(string monitorId, string key, CancellationToken cancellationToken = default)
    {
        var configuration = RequiredConfiguration(monitorId);
        var client = ClientFor(monitorId, configuration);
        await client.SendKeyAsync(key, cancellationToken).ConfigureAwait(false);

        if (client.Token != configuration.Token)
        {
            configuration.Token = client.Token;
            _store.Save(configuration);
        }
    }

    public async Task SendSequenceAsync(
        string monitorId,
        IEnumerable<(string Key, TimeSpan DelayAfter)> sequence,
        CancellationToken cancellationToken = default)
    {
        foreach (var (key, delay) in sequence)
        {
            await SendKeyAsync(monitorId, key, cancellationToken).ConfigureAwait(false);
            if (delay > TimeSpan.Zero) await Task.Delay(delay, cancellationToken).ConfigureAwait(false);
        }
    }

    public Task PowerOnAsync(string monitorId, CancellationToken cancellationToken = default)
    {
        var configuration = RequiredConfiguration(monitorId);
        if (string.IsNullOrWhiteSpace(configuration.MacAddress))
            throw new InvalidOperationException("Enter the monitor Wi-Fi MAC address before using Wake-on-LAN.");
        return WakeOnLan.SendAsync(configuration.MacAddress, cancellationToken);
    }

    SamsungTizenConfiguration RequiredConfiguration(string monitorId)
    {
        var configuration = _store.Get(monitorId);
        if (configuration is null || string.IsNullOrWhiteSpace(configuration.IpAddress))
            throw new InvalidOperationException("Associate a Samsung Tizen display first.");
        return configuration;
    }

    SamsungTizenClient ClientFor(string monitorId, SamsungTizenConfiguration configuration)
    {
        lock (_lock)
        {
            if (_clients.TryGetValue(monitorId, out var client)) return client;
            client = new SamsungTizenClient(configuration.IpAddress, configuration.Token,
                configuration.ServerCertificateFingerprint);
            _clients[monitorId] = client;
            return client;
        }
    }

    public async ValueTask DisposeAsync()
    {
        SamsungTizenClient[] clients;
        lock (_lock)
        {
            clients = _clients.Values.ToArray();
            _clients.Clear();
        }

        foreach (var client in clients) await client.DisposeAsync().ConfigureAwait(false);
        _httpClient.Dispose();
    }
}
