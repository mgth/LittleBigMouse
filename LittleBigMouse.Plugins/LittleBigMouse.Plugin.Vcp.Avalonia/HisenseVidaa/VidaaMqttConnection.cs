#nullable enable
using System.Buffers.Binary;
using System.Net.Security;
using System.Net.Sockets;
using System.Security.Authentication;
using System.Security.Cryptography.X509Certificates;
using System.Text;

namespace LittleBigMouse.Plugin.Vcp.Avalonia.HisenseVidaa;

/// <summary>Small MQTT 3.1.1/QoS-0 transport for the VIDAA broker.</summary>
internal sealed class VidaaMqttConnection : IAsyncDisposable
{
    readonly SemaphoreSlim _writeLock = new(1, 1);
    TcpClient? _tcpClient;
    Stream? _stream;
    CancellationTokenSource? _lifetime;
    Task? _readerTask;
    Task? _pingTask;
    ushort _packetId;
    volatile bool _connected;

    public event Action<string, byte[], bool>? MessageReceived;
    public bool Connected => _connected;
    public string ServerCertificateFingerprint { get; private set; } = "";

    public async Task ConnectAsync(
        string host,
        int port,
        string clientId,
        string username,
        string password,
        string certificatePath,
        string certificatePassword,
        string expectedCertificateFingerprint,
        bool allowNewCertificate,
        CancellationToken cancellationToken)
    {
        await DisposeConnectionAsync().ConfigureAwait(false);
        _tcpClient = new TcpClient(AddressFamily.InterNetwork);
        await _tcpClient.ConnectAsync(host, port, cancellationToken).ConfigureAwait(false);

        var certificatePin = new DeviceCertificatePin(
            expectedCertificateFingerprint, allowNewCertificate);
        var ssl = new SslStream(
            _tcpClient.GetStream(),
            leaveInnerStreamOpen: false,
            certificatePin.Validate);
        var certificates = new X509CertificateCollection();
        if (!string.IsNullOrWhiteSpace(certificatePath))
        {
            if (!File.Exists(certificatePath))
                throw new FileNotFoundException("The VIDAA client certificate was not found.", certificatePath);
            certificates.Add(X509CertificateLoader.LoadPkcs12FromFile(
                certificatePath, certificatePassword,
                X509KeyStorageFlags.EphemeralKeySet | X509KeyStorageFlags.Exportable));
        }

        try
        {
            await ssl.AuthenticateAsClientAsync(new SslClientAuthenticationOptions
            {
                TargetHost = host,
                ClientCertificates = certificates,
                // Several VIDAA U6 brokers advertise newer TLS but abort encrypted
                // application data after negotiating it. The official Android client
                // uses TLS 1.2 for this MQTT channel.
                EnabledSslProtocols = SslProtocols.Tls12,
                CertificateRevocationCheckMode = X509RevocationMode.NoCheck,
            }, cancellationToken).ConfigureAwait(false);
        }
        catch (AuthenticationException e) when (certificates.Count == 0)
        {
            throw new AuthenticationException(
                "VIDAA rejected TLS. Select the PKCS#12 client certificate extracted from the official app.", e);
        }

        ServerCertificateFingerprint = certificatePin.ObservedFingerprint;

        _stream = ssl;
        await WritePacketAsync(BuildConnectPacket(clientId, username, password), cancellationToken)
            .ConfigureAwait(false);
        (byte Header, byte[] Payload) connack;
        try
        {
            connack = await ReadPacketAsync(_stream, cancellationToken).ConfigureAwait(false);
        }
        catch (IOException e) when (certificates.Count == 0)
        {
            throw VidaaCertificate.MissingException(e);
        }
        if ((connack.Header >> 4) != 2 || connack.Payload.Length < 2)
            throw new IOException("VIDAA returned an invalid MQTT connection response.");
        if (connack.Payload[1] != 0)
            throw new UnauthorizedAccessException(MqttError(connack.Payload[1]));

        _connected = true;
        // The caller token limits the handshake only. Once established, MQTT has
        // its own lifetime so a completed UI command cannot tear down the session.
        _lifetime = new CancellationTokenSource();
        _readerTask = ReadLoopAsync(_lifetime.Token);
        _pingTask = PingLoopAsync(_lifetime.Token);
    }

    public async Task SubscribeAsync(IEnumerable<string> topics, CancellationToken cancellationToken)
    {
        foreach (var topic in topics)
        {
            var id = unchecked(++_packetId);
            if (id == 0) id = unchecked(++_packetId);
            using var body = new MemoryStream();
            WriteUInt16(body, id);
            WriteUtf8(body, topic);
            body.WriteByte(0);
            await WriteMqttPacketAsync(0x82, body.ToArray(), cancellationToken).ConfigureAwait(false);
        }
    }

    public async Task UnsubscribeAsync(IEnumerable<string> topics, CancellationToken cancellationToken)
    {
        foreach (var topic in topics)
        {
            var id = unchecked(++_packetId);
            if (id == 0) id = unchecked(++_packetId);
            using var body = new MemoryStream();
            WriteUInt16(body, id);
            WriteUtf8(body, topic);
            await WriteMqttPacketAsync(0xa2, body.ToArray(), cancellationToken).ConfigureAwait(false);
        }
    }

    public Task PublishAsync(string topic, string payload, CancellationToken cancellationToken)
    {
        using var body = new MemoryStream();
        WriteUtf8(body, topic);
        body.Write(Encoding.UTF8.GetBytes(payload));
        return WriteMqttPacketAsync(0x30, body.ToArray(), cancellationToken);
    }

    async Task ReadLoopAsync(CancellationToken cancellationToken)
    {
        try
        {
            while (_stream is not null)
            {
                var packet = await ReadPacketAsync(_stream, cancellationToken).ConfigureAwait(false);
                if ((packet.Header >> 4) != 3) continue;
                var span = packet.Payload.AsSpan();
                if (span.Length < 2) continue;
                var topicLength = BinaryPrimitives.ReadUInt16BigEndian(span);
                if (span.Length < 2 + topicLength) continue;
                var topic = Encoding.UTF8.GetString(span.Slice(2, topicLength));
                var offset = 2 + topicLength;
                var qos = (packet.Header >> 1) & 0x03;
                if (qos > 0) offset += 2;
                if (offset > span.Length) continue;
                MessageReceived?.Invoke(topic, span[offset..].ToArray(), (packet.Header & 0x01) != 0);
            }
        }
        catch (OperationCanceledException)
        {
        }
        catch (Exception e)
        {
            Console.Error.WriteLine($"VIDAA MQTT receive loop stopped: {e.Message}");
        }
        finally
        {
            if (!cancellationToken.IsCancellationRequested) MarkDisconnected();
        }
    }

    async Task PingLoopAsync(CancellationToken cancellationToken)
    {
        try
        {
            while (true)
            {
                await Task.Delay(TimeSpan.FromSeconds(30), cancellationToken).ConfigureAwait(false);
                await WritePacketAsync([0xc0, 0x00], cancellationToken).ConfigureAwait(false);
            }
        }
        catch (OperationCanceledException)
        {
        }
        catch (Exception e)
        {
            Console.Error.WriteLine($"VIDAA MQTT keepalive stopped: {e.Message}");
            MarkDisconnected();
        }
    }

    async Task WriteMqttPacketAsync(byte header, byte[] body, CancellationToken cancellationToken)
    {
        using var packet = new MemoryStream();
        packet.WriteByte(header);
        WriteRemainingLength(packet, body.Length);
        packet.Write(body);
        await WritePacketAsync(packet.ToArray(), cancellationToken).ConfigureAwait(false);
    }

    async Task WritePacketAsync(byte[] packet, CancellationToken cancellationToken)
    {
        var stream = _stream ?? throw new IOException("The VIDAA MQTT connection is closed.");
        await _writeLock.WaitAsync(cancellationToken).ConfigureAwait(false);
        try
        {
            await stream.WriteAsync(packet, cancellationToken).ConfigureAwait(false);
            await stream.FlushAsync(cancellationToken).ConfigureAwait(false);
        }
        catch
        {
            MarkDisconnected();
            throw;
        }
        finally
        {
            _writeLock.Release();
        }
    }

    internal static byte[] BuildConnectPacket(string clientId, string username, string password)
    {
        using var body = new MemoryStream();
        WriteUtf8(body, "MQTT");
        body.WriteByte(4); // MQTT 3.1.1
        body.WriteByte(0xc2); // username, password, clean session
        WriteUInt16(body, 60);
        WriteUtf8(body, clientId);
        WriteUtf8(body, username);
        WriteUtf8(body, password);

        using var packet = new MemoryStream();
        packet.WriteByte(0x10);
        WriteRemainingLength(packet, checked((int)body.Length));
        body.Position = 0;
        body.CopyTo(packet);
        return packet.ToArray();
    }

    static async Task<(byte Header, byte[] Payload)> ReadPacketAsync(
        Stream stream,
        CancellationToken cancellationToken)
    {
        var header = await ReadByteAsync(stream, cancellationToken).ConfigureAwait(false);
        var multiplier = 1;
        var length = 0;
        byte encoded;
        do
        {
            encoded = await ReadByteAsync(stream, cancellationToken).ConfigureAwait(false);
            length += (encoded & 127) * multiplier;
            multiplier *= 128;
            if (multiplier > 128 * 128 * 128 * 128)
                throw new IOException("Malformed MQTT remaining length.");
        } while ((encoded & 128) != 0);

        var payload = new byte[length];
        await stream.ReadExactlyAsync(payload, cancellationToken).ConfigureAwait(false);
        return (header, payload);
    }

    static async Task<byte> ReadByteAsync(Stream stream, CancellationToken cancellationToken)
    {
        var buffer = new byte[1];
        if (await stream.ReadAsync(buffer, cancellationToken).ConfigureAwait(false) != 1)
            throw new EndOfStreamException("VIDAA closed the MQTT connection.");
        return buffer[0];
    }

    static void WriteUtf8(Stream stream, string value)
    {
        var bytes = Encoding.UTF8.GetBytes(value);
        if (bytes.Length > ushort.MaxValue) throw new ArgumentOutOfRangeException(nameof(value));
        WriteUInt16(stream, (ushort)bytes.Length);
        stream.Write(bytes);
    }

    static void WriteUInt16(Stream stream, ushort value)
    {
        Span<byte> bytes = stackalloc byte[2];
        BinaryPrimitives.WriteUInt16BigEndian(bytes, value);
        stream.Write(bytes);
    }

    static void WriteRemainingLength(Stream stream, int value)
    {
        do
        {
            var digit = value % 128;
            value /= 128;
            if (value > 0) digit |= 0x80;
            stream.WriteByte((byte)digit);
        } while (value > 0);
    }

    static string MqttError(byte code) => code switch
    {
        1 => "VIDAA rejected the MQTT protocol version.",
        2 => "VIDAA rejected the generated client identifier.",
        3 => "The VIDAA MQTT service is unavailable.",
        4 => "VIDAA rejected the generated username or password.",
        5 => "VIDAA refused authorization. Check the projector date/time, MAC address and client certificate.",
        _ => $"VIDAA MQTT connection failed with code {code}.",
    };

    async Task DisposeConnectionAsync()
    {
        _connected = false;
        if (_lifetime is not null)
        {
            await _lifetime.CancelAsync().ConfigureAwait(false);
            _lifetime.Dispose();
            _lifetime = null;
        }
        if (_readerTask is not null)
        {
            try { await _readerTask.ConfigureAwait(false); } catch { }
            _readerTask = null;
        }
        if (_pingTask is not null)
        {
            try { await _pingTask.ConfigureAwait(false); } catch { }
            _pingTask = null;
        }
        if (_stream is not null)
        {
            await _stream.DisposeAsync().ConfigureAwait(false);
            _stream = null;
        }
        _tcpClient?.Dispose();
        _tcpClient = null;
    }

    void MarkDisconnected()
    {
        _connected = false;
        try { _tcpClient?.Close(); }
        catch { }
    }

    public async ValueTask DisposeAsync()
    {
        await DisposeConnectionAsync().ConfigureAwait(false);
        _writeLock.Dispose();
    }
}
