#nullable enable
using System.Collections.Concurrent;

namespace LittleBigMouse.Plugin.Vcp.Avalonia.HisenseVidaa;

/// <summary>
/// What one Hisense VIDAA device can be asked to do, over the one session it accepts.
///
/// The façade owns the layers underneath and keeps them apart: <see cref="VidaaSession"/> holds
/// the connection and reopens it, <see cref="VidaaResponseRouter"/> turns what the device
/// publishes into the answers commands wait for, and <see cref="VidaaPairing"/> settles what a
/// session needs to be accepted. What is left here is the shape of each command — its topic, its
/// payload, and how long its answer is worth waiting for — and the guarantee that only one runs
/// at a time on that shared session.
/// </summary>
public sealed class HisenseVidaaClient : IAsyncDisposable
{
    /// <summary>The device answers a settings request in one message, or not at all.</summary>
    static readonly TimeSpan PictureSettingsTimeout = TimeSpan.FromSeconds(6);

    /// <summary>A volume change is echoed on the broadcast topic almost immediately.</summary>
    static readonly TimeSpan VolumeTimeout = TimeSpan.FromSeconds(4);

    /// <summary>Everything the broker holds, for the traffic listener only.</summary>
    static readonly string[] AllTopics = ["#"];

    readonly HisenseVidaaConfiguration _configuration;
    readonly DeviceCommandGate _commands = new();
    readonly VidaaSession _session;
    readonly VidaaResponseRouter _responses;
    readonly VidaaPairing _pairing;

    public HisenseVidaaClient(HisenseVidaaConfiguration configuration)
        : this(configuration, transport: null)
    {
    }

    /// <param name="transport">
    /// Opens the MQTT connections of this client. Defaults to the real TLS transport; tests pass
    /// a simulated one to drive pairing and reconnection without a device on the network.
    /// </param>
    internal HisenseVidaaClient(
        HisenseVidaaConfiguration configuration,
        VidaaConnectionFactory? transport)
    {
        _configuration = configuration;
        _responses = new VidaaResponseRouter(configuration);
        _session = new VidaaSession(configuration, transport);
        _session.MessageReceived += _responses.Handle;
        _pairing = new VidaaPairing(configuration, _session, _responses);
    }

    public bool Connected => _session.Connected;
    public HisenseVidaaConfiguration Configuration => _configuration;

    public Task StartPairingAsync(CancellationToken cancellationToken, bool requestPin = true)
        => _pairing.StartAsync(requestPin, cancellationToken);

    public Task AuthenticateAsync(string pin, CancellationToken cancellationToken)
        => _pairing.AuthenticateAsync(pin, cancellationToken);

    public Task SendKeyAsync(string key, CancellationToken cancellationToken)
    {
        var topic = HisenseVidaaProtocol.Topic("remote_service", _configuration.ClientId, "sendkey");
        var payload = HisenseVidaaProtocol.TranslateKey(key);
        return _commands.RunExclusiveAsync(
            token => _session.SendAsync(topic, payload, token), cancellationToken);
    }

    public Task SetPictureSettingAsync(int menuId, int value, CancellationToken cancellationToken)
    {
        var topic = HisenseVidaaProtocol.Topic("platform_service", _configuration.ClientId, "picturesetting");
        var payload = HisenseVidaaProtocol.PictureSettingPayload(menuId, value);
        return _commands.RunExclusiveAsync(
            token => _session.SendAsync(topic, payload, token), cancellationToken);
    }

    public Task<IReadOnlyList<VidaaPictureSetting>> GetPictureSettingsAsync(
        CancellationToken cancellationToken)
    {
        var topic = HisenseVidaaProtocol.Topic("platform_service", _configuration.ClientId, "picturesetting");
        var payload = HisenseVidaaProtocol.PictureSettingsRequestPayload();
        return _commands.RunExclusiveAsync(async token =>
        {
            await _session.EnsureOpenAsync(token).ConfigureAwait(false);
            return await _responses.PictureSettings.CollectAsync(
                    requestToken => _session.SendAsync(topic, payload, requestToken),
                    PictureSettingsTimeout, token)
                .ConfigureAwait(false);
        }, cancellationToken);
    }

    public Task<int> GetVolumeAsync(CancellationToken cancellationToken)
        => SendVolumeActionAsync("getvolume", "0", cancellationToken);

    public Task<int> SetVolumeAsync(int volume, CancellationToken cancellationToken)
        => SendVolumeActionAsync("changevolume", HisenseVidaaProtocol.VolumePayload(volume), cancellationToken);

    public Task SendPlatformActionAsync(string action, int value, CancellationToken cancellationToken)
    {
        var topic = HisenseVidaaProtocol.Topic(
            "platform_service",
            _configuration.ClientId,
            HisenseVidaaProtocol.PlatformActionName(action));
        var payload = HisenseVidaaProtocol.ExperimentalLevelPayload(value);
        return _commands.RunExclusiveAsync(
            token => _session.SendAsync(topic, payload, token), cancellationToken);
    }

    Task<int> SendVolumeActionAsync(string action, string payload, CancellationToken cancellationToken)
    {
        var topic = HisenseVidaaProtocol.Topic("platform_service", _configuration.ClientId, action);
        return _commands.RunExclusiveAsync(async token =>
        {
            await _session.EnsureOpenAsync(token).ConfigureAwait(false);
            return await _responses.Volume.CollectAsync(
                    requestToken => _session.SendAsync(topic, payload, requestToken),
                    VolumeTimeout, token)
                .ConfigureAwait(false);
        }, cancellationToken);
    }

    /// <summary>Listens for <paramref name="duration"/>, keeping the first 200 messages.</summary>
    public async Task<IReadOnlyList<VidaaTrafficMessage>> CaptureTrafficAsync(
        TimeSpan duration,
        CancellationToken cancellationToken)
    {
        if (duration <= TimeSpan.Zero || duration > TimeSpan.FromMinutes(1))
            throw new ArgumentOutOfRangeException(nameof(duration));

        var messages = new ConcurrentQueue<VidaaTrafficMessage>();
        using var capture = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
        capture.CancelAfter(duration);
        try
        {
            await ListenTrafficAsync(message =>
            {
                if (messages.Count < 200) messages.Enqueue(message);
            }, capture.Token).ConfigureAwait(false);
        }
        catch (OperationCanceledException) when (!cancellationToken.IsCancellationRequested) { }
        return messages.ToArray();
    }

    /// <summary>
    /// Subscribes to everything the broker holds until <paramref name="cancellationToken"/> ends
    /// the listen, then leaves the session as it found it.
    /// </summary>
    public async Task ListenTrafficAsync(
        Action<VidaaTrafficMessage> onMessage,
        CancellationToken cancellationToken)
    {
        ArgumentNullException.ThrowIfNull(onMessage);
        var filter = new VidaaTrafficFilter(onMessage);

        await _commands.RunExclusiveAsync(async token =>
        {
            await _session.EnsureOpenAsync(token).ConfigureAwait(false);
            _session.MessageReceived += filter.Handle;
            try
            {
                await _session.SubscribeAsync(AllTopics, token).ConfigureAwait(false);
            }
            catch
            {
                _session.MessageReceived -= filter.Handle;
                throw;
            }
        }, cancellationToken).ConfigureAwait(false);

        try
        {
            await Task.Delay(Timeout.InfiniteTimeSpan, cancellationToken).ConfigureAwait(false);
        }
        finally
        {
            _session.MessageReceived -= filter.Handle;
            if (_session.Connected)
            {
                using var cleanup = new CancellationTokenSource(TimeSpan.FromSeconds(2));
                try { await _session.UnsubscribeAsync(AllTopics, cleanup.Token).ConfigureAwait(false); }
                catch (Exception e) when (VidaaSession.IsConnectionFailure(e)) { }
                catch (OperationCanceledException) { }
            }
        }
    }

    public async ValueTask DisposeAsync()
        => await _commands.CloseAsync(_session.CloseAsync).ConfigureAwait(false);
}
