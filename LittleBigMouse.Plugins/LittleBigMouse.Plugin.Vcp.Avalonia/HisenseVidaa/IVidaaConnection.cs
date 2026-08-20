#nullable enable

namespace LittleBigMouse.Plugin.Vcp.Avalonia.HisenseVidaa;

/// <summary>
/// An open MQTT session with a VIDAA device, as its owner uses it.
///
/// Opening is left to <see cref="VidaaConnectionFactory"/> rather than being a method here, so an
/// owner only ever holds a connection that is already established — and so the session, the
/// pairing exchange and the command façade can be driven over a simulated transport in tests
/// without a projector on the network.
/// </summary>
internal interface IVidaaConnection : IAsyncDisposable
{
    /// <summary>Topic, raw payload, and whether the broker replayed a retained message.</summary>
    event Action<string, byte[], bool>? MessageReceived;

    bool Connected { get; }

    Task SubscribeAsync(IEnumerable<string> topics, CancellationToken cancellationToken);

    Task UnsubscribeAsync(IEnumerable<string> topics, CancellationToken cancellationToken);

    Task PublishAsync(string topic, string payload, CancellationToken cancellationToken);
}

/// <summary>Everything the VIDAA broker needs to accept one session.</summary>
internal sealed record VidaaConnectionRequest(
    string Host,
    string ClientId,
    string Username,
    string Password,
    string CertificatePath,
    string CertificatePassword);

/// <summary>
/// Opens one session, or throws the way the broker refused it —
/// <see cref="UnauthorizedAccessException"/> for rejected credentials.
/// </summary>
internal delegate Task<IVidaaConnection> VidaaConnectionFactory(
    VidaaConnectionRequest request,
    CancellationToken cancellationToken);

internal static class VidaaConnections
{
    /// <summary>Opens the real TLS MQTT connection on the VIDAA remote-control port.</summary>
    public static async Task<IVidaaConnection> OpenMqttAsync(
        VidaaConnectionRequest request,
        CancellationToken cancellationToken)
    {
        var connection = new VidaaMqttConnection();
        try
        {
            await connection.ConnectAsync(
                request.Host,
                HisenseVidaaProtocol.MqttPort,
                request.ClientId,
                request.Username,
                request.Password,
                request.CertificatePath,
                request.CertificatePassword,
                cancellationToken).ConfigureAwait(false);
        }
        catch
        {
            // A refused handshake still leaves a socket and a TLS stream behind. The pairing
            // sweep tries several credentials in a row, so each failure is cleaned up at once.
            await connection.DisposeAsync().ConfigureAwait(false);
            throw;
        }
        return connection;
    }
}
