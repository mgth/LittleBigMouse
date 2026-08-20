#nullable enable

namespace LittleBigMouse.Plugin.Vcp.Avalonia.HisenseVidaa;

/// <summary>
/// Associates LittleBigMouse with one device: opens a session it accepts, has it display a PIN,
/// and trades that PIN for the tokens every later session reuses. What it settles is written to
/// the configuration, which <see cref="VidaaConnectionProfile"/> then reads to reconnect.
///
/// Which credentials a device accepts is only partly known in advance. The UPnP descriptor gives
/// a protocol generation when it answers, but not the exact spelling of the identity the broker
/// hashes, so pairing sweeps the known variants and keeps the first the device lets in.
/// </summary>
internal sealed class VidaaPairing(
    HisenseVidaaConfiguration configuration,
    VidaaSession session,
    VidaaResponseRouter responses)
{
    /// <summary>
    /// A device rejecting a credential variant answers quickly. Some brokers instead go silent
    /// for a while after a rejection, so an attempt that stops answering is abandoned rather than
    /// left to hold up the remaining variants.
    /// </summary>
    static readonly TimeSpan AttemptTimeout = TimeSpan.FromSeconds(6);

    /// <summary>The PIN is read off the screen and typed, and the tokens follow it.</summary>
    static readonly TimeSpan AnswerTimeout = TimeSpan.FromSeconds(12);

    /// <summary>
    /// Opens a session the device accepts and, unless <paramref name="requestPin"/> says the user
    /// already has a PIN from the device's own remote-control settings, asks it to display one.
    /// </summary>
    public async Task StartAsync(bool requestPin, CancellationToken cancellationToken)
    {
        if (string.IsNullOrWhiteSpace(configuration.ControllerMacAddress))
            configuration.ControllerMacAddress = NetworkIdentity.ControllerMacFor(configuration.IpAddress);

        if (HisenseVidaaProtocol.UsesStaticLegacyProtocol(configuration.ProtocolVersion))
            await StartLegacyAsync(requestPin, cancellationToken).ConfigureAwait(false);
        else
            await StartDynamicAsync(requestPin, cancellationToken).ConfigureAwait(false);
    }

    /// <summary>
    /// Sends the PIN the device displays. On RemoteNOW that is the whole pairing; newer devices
    /// answer it with an access token, which the router stores as it arrives.
    /// </summary>
    public async Task AuthenticateAsync(string pin, CancellationToken cancellationToken)
    {
        if (!session.Connected)
            throw new InvalidOperationException("The VIDAA connection is not open.");

        var legacy = HisenseVidaaProtocol.UsesStaticLegacyProtocol(configuration.ProtocolVersion);
        var pinAccepted = responses.PinAccepted.Expect();
        var tokenIssued = responses.TokenIssued.Expect();
        await session.PublishAsync(
            HisenseVidaaProtocol.Topic("ui_service", configuration.ClientId, "authenticationcode"),
            legacy ? HisenseVidaaProtocol.LegacyPinPayload(pin) : HisenseVidaaProtocol.PinPayload(pin),
            cancellationToken).ConfigureAwait(false);

        await pinAccepted.WaitAsync(AnswerTimeout, cancellationToken).ConfigureAwait(false);
        if (legacy)
        {
            configuration.LegacyAuthorized = true;
            return;
        }

        await session.PublishAsync(
            HisenseVidaaProtocol.TokenRequestTopic(configuration.ClientId),
            HisenseVidaaProtocol.TokenRequestPayload(),
            cancellationToken).ConfigureAwait(false);
        await session.PublishAsync(
            HisenseVidaaProtocol.Topic("ui_service", configuration.ClientId, "authenticationcodeclose"),
            "", cancellationToken).ConfigureAwait(false);
        await tokenIssued.WaitAsync(AnswerTimeout, cancellationToken).ConfigureAwait(false);
    }

    /// <summary>
    /// RemoteNOW devices such as the C1 have no credentials to discover: the service account is
    /// static and the device answers on a topic derived from the controller MAC.
    /// </summary>
    async Task StartLegacyAsync(bool requestPin, CancellationToken cancellationToken)
    {
        var identity = HisenseVidaaProtocol.NormalizeMac(configuration.ControllerMacAddress)
            .ToUpperInvariant();
        configuration.DeviceUuid = identity;
        configuration.AuthMethod = VidaaAuthMethod.Legacy;
        configuration.ClientId = HisenseVidaaProtocol.LegacyDeviceTopic(identity);
        configuration.MqttUsername = HisenseVidaaProtocol.LegacyMqttUsername;
        configuration.AccessToken = "";
        configuration.RefreshToken = "";
        configuration.LegacyAuthorized = false;

        await session.OpenAsync(
            VidaaConnectionProfile.LegacyCredentials(),
            VidaaConnectionProfile.ResponseTopics(configuration),
            cancellationToken).ConfigureAwait(false);

        // Armed before the request, so an acknowledgement that arrives while the user is still
        // reading the PIN off the screen is not lost. AuthenticateAsync arms its own wait.
        _ = responses.PinAccepted.Expect();
        if (requestPin)
            await session.PublishAsync(
                HisenseVidaaProtocol.Topic("ui_service", configuration.ClientId, "gettvstate"),
                "", cancellationToken).ConfigureAwait(false);
    }

    async Task StartDynamicAsync(bool requestPin, CancellationToken cancellationToken)
    {
        var identity = PairingIdentity();
        var identities = new[] { identity, identity.ToLowerInvariant(), identity.ToUpperInvariant() }
            .Distinct(StringComparer.Ordinal)
            .ToArray();
        var methods = configuration.ProtocolVersion is null
            ? new[] { VidaaAuthMethod.Modern, VidaaAuthMethod.Middle, VidaaAuthMethod.Legacy }
            : new[] { HisenseVidaaProtocol.AuthMethodFor(configuration.ProtocolVersion) };
        UnauthorizedAccessException? lastAuthorizationError = null;

        foreach (var method in methods)
            foreach (var candidate in identities)
            {
                var credentials = HisenseVidaaProtocol.GenerateCredentials(
                    candidate, method, brand: NormalizedBrand());
                try
                {
                    using var attempt = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
                    attempt.CancelAfter(AttemptTimeout);
                    // The answers come back addressed to the candidate being tried, not to whatever
                    // an earlier pairing left in the configuration: a first pairing has no client
                    // identifier there at all, and would otherwise listen on an empty one.
                    await session.OpenAsync(
                        credentials,
                        HisenseVidaaProtocol.ResponseTopics(credentials.ClientId),
                        attempt.Token).ConfigureAwait(false);
                }
                catch (UnauthorizedAccessException e)
                {
                    lastAuthorizationError = e;
                    continue;
                }
                catch (OperationCanceledException) when (!cancellationToken.IsCancellationRequested)
                {
                    // Some brokers delay a reconnect after rejecting a credential
                    // variant. Continue with the next known representation.
                    continue;
                }

                configuration.DeviceUuid = candidate;
                configuration.AuthMethod = method;
                configuration.ClientId = credentials.ClientId;
                configuration.MqttUsername = credentials.Username;
                configuration.AccessToken = "";
                configuration.RefreshToken = "";

                _ = responses.PinAccepted.Expect();
                _ = responses.TokenIssued.Expect();
                if (requestPin)
                    await session.PublishAsync(
                        HisenseVidaaProtocol.Topic("ui_service", configuration.ClientId, "vidaa_app_connect"),
                        HisenseVidaaProtocol.PairingRequestPayload(), cancellationToken).ConfigureAwait(false);
                return;
            }

        var protocol = configuration.ProtocolVersion?.ToString() ?? "unknown";
        throw new UnauthorizedAccessException(
            $"VIDAA rejected every authentication variant for protocol {protocol} and controller {identity}. " +
            "Check the projector date, time and timezone.", lastAuthorizationError);
    }

    string NormalizedBrand()
    {
        var brand = configuration.Brand.Trim().ToLowerInvariant();
        return string.IsNullOrWhiteSpace(brand) || brand.Contains("hisense", StringComparison.Ordinal)
            ? "his"
            : brand;
    }

    string PairingIdentity()
    {
        var configured = HisenseVidaaProtocol.NormalizeMac(configuration.DeviceUuid, preserveCase: true);
        if (configured.Count(Uri.IsHexDigit) == 12) return configured;
        return HisenseVidaaProtocol.NormalizeMac(configuration.ControllerMacAddress, preserveCase: true);
    }
}
