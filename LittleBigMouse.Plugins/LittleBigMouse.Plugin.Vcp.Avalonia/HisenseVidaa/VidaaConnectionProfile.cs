#nullable enable

namespace LittleBigMouse.Plugin.Vcp.Avalonia.HisenseVidaa;

/// <summary>
/// Reads a stored configuration as connection parameters: which credentials open the session, and
/// which topics the answers arrive on. Both differ between protocol generations, and the pairing
/// exchange and the automatic reconnection have to agree on them, so the rule lives in one place.
/// </summary>
internal static class VidaaConnectionProfile
{
    /// <summary>
    /// Credentials of the RemoteNOW service account, which identifies the application rather than
    /// the device: the MQTT client identifier is therefore unique per session, while
    /// <see cref="HisenseVidaaConfiguration.ClientId"/> keeps the <c>$normal</c> device topic the
    /// projector publishes on.
    /// </summary>
    public static VidaaMqttCredentials LegacyCredentials() => new(
        $"LittleBigMouse/{Guid.NewGuid():N}",
        HisenseVidaaProtocol.LegacyMqttUsername,
        HisenseVidaaProtocol.LegacyMqttPassword);

    /// <summary>Credentials that reopen the session an earlier pairing left in the configuration.</summary>
    public static VidaaMqttCredentials StoredCredentials(HisenseVidaaConfiguration configuration)
        => HisenseVidaaProtocol.UsesStaticLegacyProtocol(configuration.ProtocolVersion)
            ? LegacyCredentials()
            : new VidaaMqttCredentials(
                configuration.ClientId,
                configuration.MqttUsername,
                configuration.AccessToken);

    /// <summary>Topics carrying the answers to what this configuration is allowed to ask.</summary>
    public static IReadOnlyList<string> ResponseTopics(HisenseVidaaConfiguration configuration)
        => HisenseVidaaProtocol.UsesStaticLegacyProtocol(configuration.ProtocolVersion)
            ? HisenseVidaaProtocol.LegacyResponseTopics(configuration.ClientId)
            : HisenseVidaaProtocol.ResponseTopics(configuration.ClientId);

    public static VidaaConnectionRequest Request(
        HisenseVidaaConfiguration configuration,
        VidaaMqttCredentials credentials)
        => new(
            configuration.IpAddress,
            credentials.ClientId,
            credentials.Username,
            credentials.Password,
            configuration.ClientCertificatePath,
            configuration.ClientCertificatePassword);
}
