#nullable enable
using System.Net.WebSockets;
using System.Text;

namespace LittleBigMouse.Plugin.Vcp.Avalonia.SamsungTizen;

/// <summary>One serialized local remote-control channel to a Samsung Tizen display.</summary>
public sealed class SamsungTizenClient(
    string ipAddress,
    string token = "",
    string expectedCertificateFingerprint = "",
    bool allowNewCertificate = false) : IAsyncDisposable
{
    readonly SemaphoreSlim _gate = new(1, 1);
    ClientWebSocket? _socket;
    string _token = token;
    readonly DeviceCertificatePin _certificatePin = new(
        expectedCertificateFingerprint, allowNewCertificate);

    public string Token => _token;
    public bool Connected => _socket?.State == WebSocketState.Open;
    public string ServerCertificateFingerprint => _certificatePin.ObservedFingerprint;

    public async Task<string> ConnectAsync(CancellationToken cancellationToken = default)
    {
        await _gate.WaitAsync(cancellationToken).ConfigureAwait(false);
        try
        {
            await EnsureConnectedLockedAsync(cancellationToken).ConfigureAwait(false);
            return _token;
        }
        finally
        {
            _gate.Release();
        }
    }

    public async Task SendKeyAsync(string key, CancellationToken cancellationToken = default)
    {
        await _gate.WaitAsync(cancellationToken).ConfigureAwait(false);
        try
        {
            await EnsureConnectedLockedAsync(cancellationToken).ConfigureAwait(false);
            var payload = SamsungTizenProtocol.RemoteKeyPayload(key);
            try
            {
                await SendTextLockedAsync(payload, cancellationToken).ConfigureAwait(false);
            }
            catch (Exception e) when (e is WebSocketException or IOException or ObjectDisposedException)
            {
                ResetSocketLocked();
                await EnsureConnectedLockedAsync(cancellationToken).ConfigureAwait(false);
                await SendTextLockedAsync(payload, cancellationToken).ConfigureAwait(false);
            }
        }
        finally
        {
            _gate.Release();
        }
    }

    async Task EnsureConnectedLockedAsync(CancellationToken cancellationToken)
    {
        if (_socket?.State == WebSocketState.Open) return;
        ResetSocketLocked();

        var socket = new ClientWebSocket();
        socket.Options.KeepAliveInterval = TimeSpan.FromSeconds(20);

        socket.Options.RemoteCertificateValidationCallback = _certificatePin.Validate;
        _socket = socket;

        try
        {
            await socket.ConnectAsync(SamsungTizenProtocol.RemoteUri(ipAddress, _token), cancellationToken)
                .ConfigureAwait(false);

            while (socket.State == WebSocketState.Open)
            {
                var message = await ReceiveTextLockedAsync(cancellationToken).ConfigureAwait(false);
                var channelEvent = SamsungTizenProtocol.ParseChannelEvent(message);
                if (!string.IsNullOrEmpty(channelEvent.Error))
                    throw PairingException(channelEvent.Error);
                if (!channelEvent.Connected) continue;

                if (!string.IsNullOrWhiteSpace(channelEvent.Token)) _token = channelEvent.Token;
                return;
            }

            throw new WebSocketException("The Samsung display closed the pairing channel.");
        }
        catch (OperationCanceledException)
        {
            ResetSocketLocked();
            throw;
        }
        catch (UnauthorizedAccessException)
        {
            ResetSocketLocked();
            throw;
        }
        catch (Exception exception)
            when (exception is WebSocketException or IOException or ObjectDisposedException)
        {
            var detail = InnermostMessage(exception);
            ResetSocketLocked();
            throw new InvalidOperationException(
                $"Could not open the Samsung remote-control channel on {ipAddress}: {detail}",
                exception);
        }
    }

    static UnauthorizedAccessException PairingException(string channelEvent)
        => channelEvent switch
        {
            "ms.channel.timeOut" => new UnauthorizedAccessException(
                "The Samsung display did not authorize IP control. Enable All Settings > Connections > Network > " +
                "Expert Settings > IP Remote and Power On with Mobile, then retry pairing."),
            "ms.channel.unauthorized" => new UnauthorizedAccessException(
                "The Samsung display denied IP control. Enable All Settings > Connections > Network > " +
                "Expert Settings > IP Remote, then retry pairing."),
            _ => new UnauthorizedAccessException("The Samsung display closed or rejected the remote-control request."),
        };

    static string InnermostMessage(Exception exception)
    {
        while (exception.InnerException is not null) exception = exception.InnerException;
        return exception.Message;
    }

    async Task SendTextLockedAsync(string payload, CancellationToken cancellationToken)
    {
        var bytes = Encoding.UTF8.GetBytes(payload);
        await _socket!.SendAsync(bytes, WebSocketMessageType.Text, true, cancellationToken)
            .ConfigureAwait(false);
    }

    void ResetSocketLocked()
    {
        if (_socket is null) return;
        try { _socket.Abort(); }
        catch { }
        _socket.Dispose();
        _socket = null;
    }

    async Task<string> ReceiveTextLockedAsync(CancellationToken cancellationToken)
    {
        var buffer = new byte[4096];
        using var message = new MemoryStream();

        while (true)
        {
            var result = await _socket!.ReceiveAsync(buffer, cancellationToken).ConfigureAwait(false);
            if (result.MessageType == WebSocketMessageType.Close)
                throw new WebSocketException("The Samsung display closed the remote-control channel.");
            if (result.MessageType != WebSocketMessageType.Text) continue;

            message.Write(buffer, 0, result.Count);
            if (result.EndOfMessage) return Encoding.UTF8.GetString(message.GetBuffer(), 0, checked((int)message.Length));
        }
    }

    public async ValueTask DisposeAsync()
    {
        await _gate.WaitAsync().ConfigureAwait(false);
        try
        {
            if (_socket?.State == WebSocketState.Open)
            {
                using var timeout = new CancellationTokenSource(TimeSpan.FromSeconds(1));
                try
                {
                    await _socket.CloseAsync(WebSocketCloseStatus.NormalClosure, "LittleBigMouse closing", timeout.Token)
                        .ConfigureAwait(false);
                }
                catch
                {
                    // The monitor may already be asleep or disconnected.
                }
            }
            _socket?.Dispose();
            _socket = null;
        }
        finally
        {
            _gate.Release();
            _gate.Dispose();
        }
    }
}
