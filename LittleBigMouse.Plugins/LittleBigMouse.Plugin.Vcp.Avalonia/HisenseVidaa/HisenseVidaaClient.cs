#nullable enable
using System.Collections.Concurrent;
using System.Text;
using System.Text.Json;

namespace LittleBigMouse.Plugin.Vcp.Avalonia.HisenseVidaa;

public sealed class HisenseVidaaClient(HisenseVidaaConfiguration configuration) : IAsyncDisposable
{
    readonly HisenseVidaaConfiguration _configuration = configuration;
    readonly SemaphoreSlim _gate = new(1, 1);
    VidaaMqttConnection? _connection;
    TaskCompletionSource<bool>? _pinAccepted;
    TaskCompletionSource<bool>? _tokenIssued;
    TaskCompletionSource<int>? _volumeReceived;
    TaskCompletionSource<IReadOnlyList<VidaaPictureSetting>>? _pictureSettingsReceived;

    public bool Connected => _connection?.Connected == true;
    public HisenseVidaaConfiguration Configuration => _configuration;

    public async Task StartPairingAsync(
        CancellationToken cancellationToken,
        bool requestPin = true)
    {
        if (string.IsNullOrWhiteSpace(_configuration.ControllerMacAddress))
            _configuration.ControllerMacAddress = NetworkIdentity.ControllerMacFor(_configuration.IpAddress);

        if (HisenseVidaaProtocol.UsesStaticLegacyProtocol(_configuration.ProtocolVersion))
        {
            var legacyIdentity = HisenseVidaaProtocol.NormalizeMac(_configuration.ControllerMacAddress)
                .ToUpperInvariant();
            _configuration.DeviceUuid = legacyIdentity;
            _configuration.AuthMethod = VidaaAuthMethod.Legacy;
            _configuration.ClientId = HisenseVidaaProtocol.LegacyDeviceTopic(legacyIdentity);
            _configuration.MqttUsername = HisenseVidaaProtocol.LegacyMqttUsername;
            _configuration.AccessToken = "";
            _configuration.RefreshToken = "";
            _configuration.LegacyAuthorized = false;
            await ConnectAsync(
                HisenseVidaaProtocol.LegacyMqttPassword,
                $"LittleBigMouse/{Guid.NewGuid():N}",
                HisenseVidaaProtocol.LegacyMqttUsername,
                cancellationToken,
                allowNewCertificate: true).ConfigureAwait(false);
            _pinAccepted = NewSignal();
            if (requestPin)
                await _connection!.PublishAsync(
                    HisenseVidaaProtocol.Topic("ui_service", _configuration.ClientId, "gettvstate"),
                    "", cancellationToken).ConfigureAwait(false);
            return;
        }

        var identity = PairingIdentity();
        var identities = new[] { identity, identity.ToLowerInvariant(), identity.ToUpperInvariant() }
            .Distinct(StringComparer.Ordinal)
            .ToArray();
        var methods = _configuration.ProtocolVersion is null
            ? new[] { VidaaAuthMethod.Modern, VidaaAuthMethod.Middle, VidaaAuthMethod.Legacy }
            : new[] { HisenseVidaaProtocol.AuthMethodFor(_configuration.ProtocolVersion) };
        UnauthorizedAccessException? lastAuthorizationError = null;

        foreach (var method in methods)
        foreach (var candidate in identities)
        {
            var credentials = HisenseVidaaProtocol.GenerateCredentials(
                candidate, method, brand: NormalizedBrand());
            try
            {
                using var attempt = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
                attempt.CancelAfter(TimeSpan.FromSeconds(6));
                await ConnectAsync(credentials.Password, credentials.ClientId, credentials.Username,
                    attempt.Token, allowNewCertificate: true)
                    .ConfigureAwait(false);
                _configuration.DeviceUuid = candidate;
                _configuration.AuthMethod = method;
                _configuration.ClientId = credentials.ClientId;
                _configuration.MqttUsername = credentials.Username;
                _configuration.AccessToken = "";
                _configuration.RefreshToken = "";
                goto Connected;
            }
            catch (UnauthorizedAccessException e)
            {
                lastAuthorizationError = e;
            }
            catch (OperationCanceledException) when (!cancellationToken.IsCancellationRequested)
            {
                // Some brokers delay a reconnect after rejecting a credential
                // variant. Continue with the next known representation.
            }
        }

        var protocol = _configuration.ProtocolVersion?.ToString() ?? "unknown";
        throw new UnauthorizedAccessException(
            $"VIDAA rejected every authentication variant for protocol {protocol} and controller {identity}. " +
            "Check the projector date, time and timezone.", lastAuthorizationError);

        Connected:

        _pinAccepted = NewSignal();
        _tokenIssued = NewSignal();
        if (requestPin)
            await _connection!.PublishAsync(
                HisenseVidaaProtocol.Topic("ui_service", _configuration.ClientId, "vidaa_app_connect"),
                HisenseVidaaProtocol.PairingRequestPayload(), cancellationToken).ConfigureAwait(false);
    }

    public async Task AuthenticateAsync(string pin, CancellationToken cancellationToken)
    {
        if (_connection is null || !_connection.Connected)
            throw new InvalidOperationException("The VIDAA connection is not open.");

        _pinAccepted = NewSignal();
        _tokenIssued = NewSignal();
        await _connection.PublishAsync(
            HisenseVidaaProtocol.Topic("ui_service", _configuration.ClientId, "authenticationcode"),
            HisenseVidaaProtocol.UsesStaticLegacyProtocol(_configuration.ProtocolVersion)
                ? HisenseVidaaProtocol.LegacyPinPayload(pin)
                : HisenseVidaaProtocol.PinPayload(pin), cancellationToken).ConfigureAwait(false);

        await _pinAccepted.Task.WaitAsync(TimeSpan.FromSeconds(12), cancellationToken).ConfigureAwait(false);
        if (HisenseVidaaProtocol.UsesStaticLegacyProtocol(_configuration.ProtocolVersion))
        {
            _configuration.LegacyAuthorized = true;
            return;
        }
        await _connection.PublishAsync(
            $"/remoteapp/tv/platform_service/{_configuration.ClientId}/data/gettoken",
            "{\"refreshtoken\":\"\"}", cancellationToken).ConfigureAwait(false);
        await _connection.PublishAsync(
            HisenseVidaaProtocol.Topic("ui_service", _configuration.ClientId, "authenticationcodeclose"),
            "", cancellationToken).ConfigureAwait(false);
        await _tokenIssued.Task.WaitAsync(TimeSpan.FromSeconds(12), cancellationToken).ConfigureAwait(false);
    }

    public async Task SendKeyAsync(string key, CancellationToken cancellationToken)
    {
        await _gate.WaitAsync(cancellationToken).ConfigureAwait(false);
        try
        {
            var topic = HisenseVidaaProtocol.Topic("remote_service", _configuration.ClientId, "sendkey");
            var payload = HisenseVidaaProtocol.TranslateKey(key);
            await EnsurePairedConnectionAsync(cancellationToken).ConfigureAwait(false);
            try
            {
                await _connection!.PublishAsync(topic, payload, cancellationToken).ConfigureAwait(false);
            }
            catch (Exception e) when (IsConnectionFailure(e))
            {
                // TcpClient.Connected can remain true until the first failed write.
                // Reopen the persistent MQTT session and retry this key once.
                await ResetConnectionAsync().ConfigureAwait(false);
                await EnsurePairedConnectionAsync(cancellationToken).ConfigureAwait(false);
                await _connection!.PublishAsync(topic, payload, cancellationToken).ConfigureAwait(false);
            }
        }
        finally
        {
            _gate.Release();
        }
    }

    public async Task SetPictureSettingAsync(int menuId, int value, CancellationToken cancellationToken)
    {
        var topic = HisenseVidaaProtocol.Topic("platform_service", _configuration.ClientId, "picturesetting");
        var payload = HisenseVidaaProtocol.PictureSettingPayload(menuId, value);
        await _gate.WaitAsync(cancellationToken).ConfigureAwait(false);
        try
        {
            await EnsurePairedConnectionAsync(cancellationToken).ConfigureAwait(false);
            try
            {
                await _connection!.PublishAsync(topic, payload, cancellationToken).ConfigureAwait(false);
            }
            catch (Exception e) when (IsConnectionFailure(e))
            {
                await ResetConnectionAsync().ConfigureAwait(false);
                await EnsurePairedConnectionAsync(cancellationToken).ConfigureAwait(false);
                await _connection!.PublishAsync(topic, payload, cancellationToken).ConfigureAwait(false);
            }
        }
        finally
        {
            _gate.Release();
        }
    }

    public async Task<IReadOnlyList<VidaaPictureSetting>> GetPictureSettingsAsync(
        CancellationToken cancellationToken)
    {
        await _gate.WaitAsync(cancellationToken).ConfigureAwait(false);
        try
        {
            await EnsurePairedConnectionAsync(cancellationToken).ConfigureAwait(false);
            var signal = new TaskCompletionSource<IReadOnlyList<VidaaPictureSetting>>(
                TaskCreationOptions.RunContinuationsAsynchronously);
            _pictureSettingsReceived = signal;
            var topic = HisenseVidaaProtocol.Topic(
                "platform_service", _configuration.ClientId, "picturesetting");
            try
            {
                try
                {
                    await _connection!.PublishAsync(
                        topic,
                        HisenseVidaaProtocol.PictureSettingsRequestPayload(),
                        cancellationToken).ConfigureAwait(false);
                }
                catch (Exception e) when (IsConnectionFailure(e))
                {
                    await ResetConnectionAsync().ConfigureAwait(false);
                    await EnsurePairedConnectionAsync(cancellationToken).ConfigureAwait(false);
                    await _connection!.PublishAsync(
                        topic,
                        HisenseVidaaProtocol.PictureSettingsRequestPayload(),
                        cancellationToken).ConfigureAwait(false);
                }
                return await signal.Task.WaitAsync(TimeSpan.FromSeconds(6), cancellationToken)
                    .ConfigureAwait(false);
            }
            finally
            {
                if (ReferenceEquals(_pictureSettingsReceived, signal)) _pictureSettingsReceived = null;
            }
        }
        finally
        {
            _gate.Release();
        }
    }

    public Task<int> GetVolumeAsync(CancellationToken cancellationToken)
        => SendVolumeActionAsync("getvolume", "0", cancellationToken);

    public Task<int> SetVolumeAsync(int volume, CancellationToken cancellationToken)
        => SendVolumeActionAsync("changevolume", HisenseVidaaProtocol.VolumePayload(volume), cancellationToken);

    public async Task SendPlatformActionAsync(string action, int value, CancellationToken cancellationToken)
    {
        var topic = HisenseVidaaProtocol.Topic(
            "platform_service",
            _configuration.ClientId,
            HisenseVidaaProtocol.PlatformActionName(action));
        var payload = HisenseVidaaProtocol.ExperimentalLevelPayload(value);
        await _gate.WaitAsync(cancellationToken).ConfigureAwait(false);
        try
        {
            await EnsurePairedConnectionAsync(cancellationToken).ConfigureAwait(false);
            try
            {
                await _connection!.PublishAsync(topic, payload, cancellationToken).ConfigureAwait(false);
            }
            catch (Exception e) when (IsConnectionFailure(e))
            {
                await ResetConnectionAsync().ConfigureAwait(false);
                await EnsurePairedConnectionAsync(cancellationToken).ConfigureAwait(false);
                await _connection!.PublishAsync(topic, payload, cancellationToken).ConfigureAwait(false);
            }
        }
        finally
        {
            _gate.Release();
        }
    }

    async Task<int> SendVolumeActionAsync(string action, string payload, CancellationToken cancellationToken)
    {
        await _gate.WaitAsync(cancellationToken).ConfigureAwait(false);
        try
        {
            await EnsurePairedConnectionAsync(cancellationToken).ConfigureAwait(false);
            var signal = new TaskCompletionSource<int>(TaskCreationOptions.RunContinuationsAsynchronously);
            _volumeReceived = signal;
            var topic = HisenseVidaaProtocol.Topic("platform_service", _configuration.ClientId, action);
            try
            {
                try
                {
                    await _connection!.PublishAsync(topic, payload, cancellationToken).ConfigureAwait(false);
                }
                catch (Exception e) when (IsConnectionFailure(e))
                {
                    await ResetConnectionAsync().ConfigureAwait(false);
                    await EnsurePairedConnectionAsync(cancellationToken).ConfigureAwait(false);
                    await _connection!.PublishAsync(topic, payload, cancellationToken).ConfigureAwait(false);
                }
                return await signal.Task.WaitAsync(TimeSpan.FromSeconds(4), cancellationToken).ConfigureAwait(false);
            }
            finally
            {
                if (ReferenceEquals(_volumeReceived, signal)) _volumeReceived = null;
            }
        }
        finally
        {
            _gate.Release();
        }
    }

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

    public async Task ListenTrafficAsync(
        Action<VidaaTrafficMessage> onMessage,
        CancellationToken cancellationToken)
    {
        ArgumentNullException.ThrowIfNull(onMessage);
        VidaaMqttConnection connection;
        var recentMessages = new Dictionary<string, DateTimeOffset>(StringComparer.Ordinal);

        void Capture(string topic, byte[] bytes, bool retained)
        {
            if (retained) return;
            var payload = VidaaTrafficCapture.DecodePayload(bytes);
            if (VidaaTrafficCapture.IsSensitive(topic, payload)) return;
            var now = DateTimeOffset.Now;
            var fingerprint = topic + "\n" + payload;
            lock (recentMessages)
            {
                if (recentMessages.TryGetValue(fingerprint, out var previous)
                    && now - previous < TimeSpan.FromMilliseconds(100)) return;
                recentMessages[fingerprint] = now;
                if (recentMessages.Count > 512)
                    foreach (var stale in recentMessages
                                 .Where(pair => now - pair.Value > TimeSpan.FromSeconds(5))
                                 .Select(pair => pair.Key)
                                 .ToArray())
                        recentMessages.Remove(stale);
            }
            onMessage(new VidaaTrafficMessage(now, topic, payload));
        }

        await _gate.WaitAsync(cancellationToken).ConfigureAwait(false);
        try
        {
            await EnsurePairedConnectionAsync(cancellationToken).ConfigureAwait(false);
            connection = _connection!;
            connection.MessageReceived += Capture;
            try
            {
                await connection.SubscribeAsync(["#"], cancellationToken).ConfigureAwait(false);
            }
            catch
            {
                connection.MessageReceived -= Capture;
                throw;
            }
        }
        finally
        {
            _gate.Release();
        }

        try
        {
            await Task.Delay(Timeout.InfiniteTimeSpan, cancellationToken).ConfigureAwait(false);
        }
        finally
        {
            connection.MessageReceived -= Capture;
            if (connection.Connected)
            {
                using var cleanup = new CancellationTokenSource(TimeSpan.FromSeconds(2));
                try { await connection.UnsubscribeAsync(["#"], cleanup.Token).ConfigureAwait(false); }
                catch (Exception e) when (IsConnectionFailure(e)) { }
                catch (OperationCanceledException) { }
            }
        }

    }

    async Task EnsurePairedConnectionAsync(CancellationToken cancellationToken)
    {
        if (_connection?.Connected == true) return;
        if (!_configuration.HasPairing)
            throw new InvalidOperationException("Pair this Hisense VIDAA device first.");
        if (HisenseVidaaProtocol.UsesStaticLegacyProtocol(_configuration.ProtocolVersion))
        {
            await ConnectAsync(
                HisenseVidaaProtocol.LegacyMqttPassword,
                $"LittleBigMouse/{Guid.NewGuid():N}",
                HisenseVidaaProtocol.LegacyMqttUsername,
                cancellationToken).ConfigureAwait(false);
            return;
        }
        await ConnectAsync(
            _configuration.AccessToken,
            _configuration.ClientId,
            _configuration.MqttUsername,
            cancellationToken).ConfigureAwait(false);
    }

    async Task ConnectAsync(
        string password,
        string clientId,
        string username,
        CancellationToken cancellationToken,
        bool allowNewCertificate = false)
    {
        await ResetConnectionAsync().ConfigureAwait(false);
        _connection = new VidaaMqttConnection();
        _connection.MessageReceived += OnMessage;
        await _connection.ConnectAsync(
            _configuration.IpAddress,
            HisenseVidaaProtocol.MqttPort,
            clientId,
            username,
            password,
            _configuration.ClientCertificatePath,
            _configuration.ClientCertificatePassword,
            _configuration.ServerCertificateFingerprint,
            allowNewCertificate,
            cancellationToken).ConfigureAwait(false);
        if (allowNewCertificate)
            _configuration.ServerCertificateFingerprint =
                _connection.ServerCertificateFingerprint;
        var topics = HisenseVidaaProtocol.UsesStaticLegacyProtocol(_configuration.ProtocolVersion)
            ? HisenseVidaaProtocol.LegacyResponseTopics(_configuration.ClientId)
            : HisenseVidaaProtocol.ResponseTopics(_configuration.ClientId);
        await _connection.SubscribeAsync(
            topics, cancellationToken)
            .ConfigureAwait(false);
    }

    async Task ResetConnectionAsync()
    {
        if (_connection is null) return;
        _connection.MessageReceived -= OnMessage;
        await _connection.DisposeAsync().ConfigureAwait(false);
        _connection = null;
    }

    static bool IsConnectionFailure(Exception exception)
        => exception is IOException or System.Net.Sockets.SocketException or ObjectDisposedException;

    void OnMessage(string topic, byte[] bytes, bool retained)
    {
        try
        {
            var payload = Encoding.UTF8.GetString(bytes);
            if (HisenseVidaaProtocol.TryParsePictureSettings(topic, payload, out var settings))
            {
                _pictureSettingsReceived?.TrySetResult(settings);
                return;
            }
            if (HisenseVidaaProtocol.TryParseVolume(topic, payload, out var volume))
            {
                _volumeReceived?.TrySetResult(volume);
                return;
            }
            if (topic.Contains("tokenissuance", StringComparison.OrdinalIgnoreCase))
            {
                using var json = JsonDocument.Parse(bytes);
                var root = json.RootElement;
                if (!root.TryGetProperty("accesstoken", out var accessToken)) return;
                _configuration.AccessToken = accessToken.GetString() ?? "";
                _configuration.RefreshToken = root.TryGetProperty("refreshtoken", out var refreshToken)
                    ? refreshToken.GetString() ?? ""
                    : "";
                var now = DateTimeOffset.UtcNow;
                _configuration.AccessTokenIssuedAt = UnixTimeOrNow(root, "accesstoken_time", now);
                _configuration.RefreshTokenIssuedAt = UnixTimeOrNow(root, "refreshtoken_time", now);
                _configuration.AccessTokenDurationDays = IntegerOr(root, "accesstoken_duration_day", 7);
                _configuration.RefreshTokenDurationDays = IntegerOr(root, "refreshtoken_duration_day", 30);
                _tokenIssued?.TrySetResult(true);
                return;
            }

            if (!topic.Contains("authentication", StringComparison.OrdinalIgnoreCase)) return;
            if (bytes.Length == 0)
            {
                _pinAccepted?.TrySetResult(true);
                return;
            }
            using var response = JsonDocument.Parse(bytes);
            var result = response.RootElement;
            if (result.ValueKind != JsonValueKind.Object) return;
            if (result.TryGetProperty("result", out var accepted) && accepted.TryGetInt32(out var code))
            {
                if (code == 1) _pinAccepted?.TrySetResult(true);
                else _pinAccepted?.TrySetException(new UnauthorizedAccessException("The VIDAA PIN was rejected."));
            }
            else if (result.TryGetProperty("statetype", out var state)
                     && state.GetString() == "authenticationcode")
            {
                // The PIN dialog is visible; AuthenticateAsync will supply the code.
            }
        }
        catch (Exception e)
        {
            Console.Error.WriteLine($"Invalid VIDAA response on {topic}: {e.Message}");
        }
    }

    string NormalizedBrand()
    {
        var brand = _configuration.Brand.Trim().ToLowerInvariant();
        return string.IsNullOrWhiteSpace(brand) || brand.Contains("hisense", StringComparison.Ordinal)
            ? "his"
            : brand;
    }

    string PairingIdentity()
    {
        var configured = HisenseVidaaProtocol.NormalizeMac(_configuration.DeviceUuid, preserveCase: true);
        if (configured.Count(Uri.IsHexDigit) == 12) return configured;
        return HisenseVidaaProtocol.NormalizeMac(_configuration.ControllerMacAddress, preserveCase: true);
    }

    static TaskCompletionSource<bool> NewSignal()
        => new(TaskCreationOptions.RunContinuationsAsynchronously);

    static int IntegerOr(JsonElement root, string property, int fallback)
        => root.TryGetProperty(property, out var value) && value.TryGetInt32(out var result) ? result : fallback;

    static DateTimeOffset UnixTimeOrNow(JsonElement root, string property, DateTimeOffset fallback)
        => root.TryGetProperty(property, out var value) && value.TryGetInt64(out var seconds)
            ? DateTimeOffset.FromUnixTimeSeconds(seconds)
            : fallback;

    public async ValueTask DisposeAsync()
    {
        await _gate.WaitAsync().ConfigureAwait(false);
        try
        {
            await ResetConnectionAsync().ConfigureAwait(false);
        }
        finally
        {
            _gate.Release();
            _gate.Dispose();
        }
    }
}
