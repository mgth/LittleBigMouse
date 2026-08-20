#nullable enable
using System.Net.Sockets;

namespace LittleBigMouse.Plugin.Vcp.Avalonia.HisenseVidaa;

/// <summary>
/// Owns the single MQTT connection a VIDAA device accepts, and everything that keeps it usable:
/// opening it with a set of credentials, subscribing the topics the answers arrive on, reopening
/// it when a command discovers the broker dropped it, and closing it exactly once.
///
/// It knows nothing of pairing or of what the messages mean. Callers hand it credentials and
/// topics — <see cref="VidaaConnectionProfile"/> derives both from the configuration — and read
/// the traffic through <see cref="MessageReceived"/>, which survives a reconnection because it
/// belongs to the session rather than to the connection of the moment.
/// </summary>
internal sealed class VidaaSession(
    HisenseVidaaConfiguration configuration,
    VidaaConnectionFactory? transport = null)
{
    readonly VidaaConnectionFactory _transport = transport ?? VidaaConnections.OpenMqttAsync;
    IVidaaConnection? _connection;

    /// <summary>Every message of the open connection, retained ones included.</summary>
    public event Action<string, byte[], bool>? MessageReceived;

    public bool Connected => _connection?.Connected == true;

    /// <summary>
    /// Drops whatever session is open and establishes a new one. The broker accepts one
    /// persistent session at a time, so the old one goes before the new one is asked for.
    /// </summary>
    public async Task OpenAsync(
        VidaaMqttCredentials credentials,
        IReadOnlyList<string> topics,
        CancellationToken cancellationToken)
    {
        await CloseAsync().ConfigureAwait(false);
        var connection = await _transport(
                VidaaConnectionProfile.Request(configuration, credentials), cancellationToken)
            .ConfigureAwait(false);
        connection.MessageReceived += Dispatch;
        _connection = connection;
        await connection.SubscribeAsync(topics, cancellationToken).ConfigureAwait(false);
    }

    /// <summary>Opens the session the stored pairing describes, unless one is already open.</summary>
    /// <exception cref="InvalidOperationException">The device has never been paired.</exception>
    public async Task EnsureOpenAsync(CancellationToken cancellationToken)
    {
        if (Connected) return;
        if (!configuration.HasPairing)
            throw new InvalidOperationException("Pair this Hisense VIDAA device first.");

        await OpenAsync(
            VidaaConnectionProfile.StoredCredentials(configuration),
            VidaaConnectionProfile.ResponseTopics(configuration),
            cancellationToken).ConfigureAwait(false);
    }

    /// <summary>
    /// Publishes on the paired session, opening it if needed and giving the command a second
    /// chance when the write is what reveals the broker dropped the previous one. See
    /// <see cref="StaleConnectionRetry"/>; here a dead session has to be closed before the
    /// stored pairing can open another.
    /// </summary>
    public async Task SendAsync(string topic, string payload, CancellationToken cancellationToken)
    {
        await EnsureOpenAsync(cancellationToken).ConfigureAwait(false);
        await StaleConnectionRetry.SendAsync(
            token => PublishAsync(topic, payload, token),
            ReopenAsync,
            IsConnectionFailure,
            cancellationToken).ConfigureAwait(false);
    }

    /// <summary>Publishes on the open session, without reopening it.</summary>
    /// <exception cref="InvalidOperationException">No session is open.</exception>
    public Task PublishAsync(string topic, string payload, CancellationToken cancellationToken)
        => Open().PublishAsync(topic, payload, cancellationToken);

    /// <inheritdoc cref="PublishAsync"/>
    public Task SubscribeAsync(IReadOnlyList<string> topics, CancellationToken cancellationToken)
        => Open().SubscribeAsync(topics, cancellationToken);

    /// <inheritdoc cref="PublishAsync"/>
    public Task UnsubscribeAsync(IReadOnlyList<string> topics, CancellationToken cancellationToken)
        => Open().UnsubscribeAsync(topics, cancellationToken);

    public async Task CloseAsync()
    {
        if (_connection is not { } connection) return;
        _connection = null;
        connection.MessageReceived -= Dispatch;
        await connection.DisposeAsync().ConfigureAwait(false);
    }

    /// <summary>
    /// Tells a session the broker dropped from a command the device refused, so that only the
    /// former is worth sending again.
    /// </summary>
    public static bool IsConnectionFailure(Exception exception)
        => exception is IOException or SocketException or ObjectDisposedException;

    async Task ReopenAsync(CancellationToken cancellationToken)
    {
        await CloseAsync().ConfigureAwait(false);
        await EnsureOpenAsync(cancellationToken).ConfigureAwait(false);
    }

    IVidaaConnection Open()
        => _connection ?? throw new InvalidOperationException("The VIDAA connection is not open.");

    void Dispatch(string topic, byte[] payload, bool retained)
        => MessageReceived?.Invoke(topic, payload, retained);
}
