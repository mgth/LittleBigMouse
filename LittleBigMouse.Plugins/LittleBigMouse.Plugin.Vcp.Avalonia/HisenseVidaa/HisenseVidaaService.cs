#nullable enable
using System.Net;

namespace LittleBigMouse.Plugin.Vcp.Avalonia.HisenseVidaa;

public sealed class HisenseVidaaService : IHisenseVidaaService, IAsyncDisposable
{
    readonly HisenseVidaaSettingsStore _store; readonly HisenseVidaaDiscovery _discovery; readonly HttpClient _http;
    readonly object _clientLock = new();
    readonly Dictionary<string, HisenseVidaaClient> _clients = [];
    public HisenseVidaaService(HisenseVidaaSettingsStore store) { _store=store; _http=new HttpClient { Timeout=TimeSpan.FromSeconds(5) }; _discovery=new HisenseVidaaDiscovery(_http); }
    public HisenseVidaaConfiguration? GetConfiguration(string id)
    {
        var configuration = _store.Get(id);
        if (configuration is null || string.IsNullOrWhiteSpace(configuration.IpAddress)
            || !string.IsNullOrWhiteSpace(configuration.ControllerMacAddress)) return configuration;
        try
        {
            configuration.ControllerMacAddress = NetworkIdentity.ControllerMacFor(configuration.IpAddress);
            _store.Save(configuration);
        }
        catch
        {
            // The route may not exist yet; Save/Request PIN will retry when the device is reachable.
        }
        return configuration;
    }
    public async Task<IReadOnlyList<HisenseVidaaDevice>> DiscoverAsync(CancellationToken cancellationToken = default)
    {
        return await _discovery.DiscoverAsync(TimeSpan.FromSeconds(3), cancellationToken);
    }
    public Task<HisenseVidaaDevice> ProbeAsync(string ipAddress, CancellationToken cancellationToken = default)
        => _discovery.ProbeAsync(ipAddress.Trim(), cancellationToken);
    public Task<HisenseVidaaDevice> FindAsync(string ipAddress, CancellationToken cancellationToken = default)
        => _discovery.FindAsync(ipAddress.Trim(), cancellationToken);
    public void SaveAddress(string id, string ip, string mac, string uuid, string certificatePath)
    {
        if (!IPAddress.TryParse(ip.Trim(), out var address) || address.AddressFamily != System.Net.Sockets.AddressFamily.InterNetwork) throw new ArgumentException("Enter a valid IPv4 address.");
        var c=_store.Get(id) ?? new HisenseVidaaConfiguration { MonitorId=id };
        if (!c.IpAddress.Equals(ip.Trim(), StringComparison.Ordinal))
        {
            c.ServerCertificateFingerprint = "";
            c.ClientId = "";
            c.MqttUsername = "";
            c.AccessToken = "";
            c.RefreshToken = "";
            c.LegacyAuthorized = false;
        }
        c.IpAddress=ip.Trim(); c.MacAddress=mac.Trim(); c.DeviceUuid=uuid.Trim();
        c.ClientCertificatePath = VidaaCertificate.Resolve(certificatePath);
        if (string.IsNullOrWhiteSpace(c.ClientCertificatePassword)) c.ClientCertificatePassword = VidaaCertificate.DefaultPassword;
        c.ControllerMacAddress = NetworkIdentity.ControllerMacFor(c.IpAddress);
        _store.Save(c);
    }
    public async Task SendKeyAsync(string id, string key, CancellationToken ct = default)
    {
        var configuration = Required(id);
        var client = await ClientForAsync(id, configuration).ConfigureAwait(false);
        await client.SendKeyAsync(key, ct).ConfigureAwait(false);
    }
    public async Task SetPictureSettingAsync(string id, int menuId, int value, CancellationToken ct = default)
    {
        var configuration = Required(id);
        var client = await ClientForAsync(id, configuration).ConfigureAwait(false);
        await client.SetPictureSettingAsync(menuId, value, ct).ConfigureAwait(false);
    }
    public async Task<IReadOnlyList<VidaaPictureSetting>> GetPictureSettingsAsync(
        string id,
        CancellationToken ct = default)
    {
        var configuration = Required(id);
        var client = await ClientForAsync(id, configuration).ConfigureAwait(false);
        return await client.GetPictureSettingsAsync(ct).ConfigureAwait(false);
    }
    public async Task<int> GetVolumeAsync(string id, CancellationToken ct = default)
    {
        var configuration = Required(id);
        var client = await ClientForAsync(id, configuration).ConfigureAwait(false);
        return await client.GetVolumeAsync(ct).ConfigureAwait(false);
    }
    public async Task<int> SetVolumeAsync(string id, int volume, CancellationToken ct = default)
    {
        var configuration = Required(id);
        var client = await ClientForAsync(id, configuration).ConfigureAwait(false);
        return await client.SetVolumeAsync(volume, ct).ConfigureAwait(false);
    }
    public async Task SendPlatformActionAsync(string id, string action, int value, CancellationToken ct = default)
    {
        var configuration = Required(id);
        var client = await ClientForAsync(id, configuration).ConfigureAwait(false);
        await client.SendPlatformActionAsync(action, value, ct).ConfigureAwait(false);
    }
    public void SaveKeyMacro(string id, string sequence)
    {
        RemoteMacro.Parse(sequence);
        var configuration = Required(id);
        configuration.KeyMacro = sequence.Trim();
        _store.Save(configuration);
    }
    public async Task SendSequenceAsync(
        string id,
        IEnumerable<(string Key, TimeSpan DelayAfter)> sequence,
        CancellationToken ct = default)
    {
        foreach (var (key, delay) in sequence)
        {
            await SendKeyAsync(id, key, ct).ConfigureAwait(false);
            if (delay > TimeSpan.Zero) await Task.Delay(delay, ct).ConfigureAwait(false);
        }
    }
    public async Task<IReadOnlyList<VidaaTrafficMessage>> CaptureTrafficAsync(
        string id,
        TimeSpan duration,
        CancellationToken ct = default)
    {
        var configuration = Required(id);
        var client = await ClientForAsync(id, configuration).ConfigureAwait(false);
        return await client.CaptureTrafficAsync(duration, ct).ConfigureAwait(false);
    }
    public async Task ListenTrafficAsync(
        string id,
        Action<VidaaTrafficMessage> onMessage,
        CancellationToken ct = default)
    {
        var configuration = Required(id);
        var client = await ClientForAsync(id, configuration).ConfigureAwait(false);
        await client.ListenTrafficAsync(onMessage, ct).ConfigureAwait(false);
    }
    public async Task RequestPinAsync(string id, CancellationToken ct = default)
    {
        var c = Required(id);
        await PreparePairingAsync(c, ct).ConfigureAwait(false);
        var client = new HisenseVidaaClient(c);
        try { await client.StartPairingAsync(ct).ConfigureAwait(false); }
        catch { await client.DisposeAsync(); throw; }
        await ReplaceClientAsync(id, client).ConfigureAwait(false);
        _store.Save(c);
    }
    public async Task PairAsync(string id, string pin, CancellationToken ct = default)
    {
        var c = Required(id);
        var client = GetClient(id);
        if (client is null || !client.Connected)
        {
            await PreparePairingAsync(c, ct).ConfigureAwait(false);
            var replacement = new HisenseVidaaClient(c);
            try
            {
                // The projector can generate a PIN from its own Remote Control
                // settings. Connect without asking it to replace that PIN.
                await replacement.StartPairingAsync(ct, requestPin: false).ConfigureAwait(false);
            }
            catch
            {
                await replacement.DisposeAsync();
                throw;
            }
            await ReplaceClientAsync(id, replacement).ConfigureAwait(false);
            client = replacement;
        }
        await client.AuthenticateAsync(pin, ct); _store.Save(client.Configuration);
    }

    async Task PreparePairingAsync(HisenseVidaaConfiguration c, CancellationToken ct)
    {
        c.ClientCertificatePath = VidaaCertificate.Resolve(c.ClientCertificatePath);
        if (string.IsNullOrWhiteSpace(c.ClientCertificatePath)) throw VidaaCertificate.MissingException();
        if (string.IsNullOrWhiteSpace(c.ClientCertificatePassword))
            c.ClientCertificatePassword = VidaaCertificate.DefaultPassword;
        c.ControllerMacAddress = NetworkIdentity.ControllerMacFor(c.IpAddress);
        try
        {
            var device = await _discovery.FindAsync(c.IpAddress, ct).ConfigureAwait(false);
            c.IpAddress = device.IpAddress;
            c.ProtocolVersion = device.ProtocolVersion;
            c.Brand = device.Brand;
        }
        catch (HttpRequestException) when (c.ProtocolVersion is null
                                           && c.MonitorId.StartsWith("HEC002F", StringComparison.OrdinalIgnoreCase))
        {
            // Hisense C1 EDID. Its descriptor advertises RemoteNOW protocol 2160,
            // but the UPnP HTTP endpoint can disappear while MQTT remains usable.
            c.ProtocolVersion = 2160;
            c.Brand = "his";
        }
        _store.Save(c);
    }
    public Task PowerOnAsync(string id, CancellationToken ct = default)
    {
        var c=Required(id); if (string.IsNullOrWhiteSpace(c.MacAddress)) throw new InvalidOperationException("Enter the projector Wi-Fi MAC address first."); return WakeOnLan.SendAsync(c.MacAddress, ct);
    }
    HisenseVidaaConfiguration Required(string id) => _store.Get(id) is { } c && !string.IsNullOrWhiteSpace(c.IpAddress) ? c : throw new InvalidOperationException("Associate a Hisense VIDAA projector first.");

    async Task<HisenseVidaaClient> ClientForAsync(string id, HisenseVidaaConfiguration configuration)
    {
        HisenseVidaaClient? previous = null;
        HisenseVidaaClient client;
        lock (_clientLock)
        {
            if (_clients.TryGetValue(id, out client!)
                && SameConnection(client.Configuration, configuration)) return client;

            previous = client;
            client = new HisenseVidaaClient(configuration);
            _clients[id] = client;
        }

        if (previous is not null) await previous.DisposeAsync().ConfigureAwait(false);
        return client;
    }

    HisenseVidaaClient? GetClient(string id)
    {
        lock (_clientLock) return _clients.GetValueOrDefault(id);
    }

    async Task ReplaceClientAsync(string id, HisenseVidaaClient client)
    {
        HisenseVidaaClient? previous;
        lock (_clientLock)
        {
            _clients.Remove(id, out previous);
            _clients[id] = client;
        }
        if (previous is not null && !ReferenceEquals(previous, client))
            await previous.DisposeAsync().ConfigureAwait(false);
    }

    static bool SameConnection(HisenseVidaaConfiguration left, HisenseVidaaConfiguration right)
        => left.IpAddress == right.IpAddress
           && left.ProtocolVersion == right.ProtocolVersion
           && left.ClientId == right.ClientId
           && left.MqttUsername == right.MqttUsername
           && left.AccessToken == right.AccessToken
           && left.LegacyAuthorized == right.LegacyAuthorized
           && left.ClientCertificatePath == right.ClientCertificatePath
           && left.ClientCertificatePassword == right.ClientCertificatePassword;

    public async ValueTask DisposeAsync()
    {
        HisenseVidaaClient[] clients;
        lock (_clientLock)
        {
            clients = _clients.Values.ToArray();
            _clients.Clear();
        }
        foreach (var client in clients) await client.DisposeAsync().ConfigureAwait(false);
        _http.Dispose();
    }
}
