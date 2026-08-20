using System.Net;
using System.Net.Sockets;

namespace LittleBigMouse.Plugin.Vcp.Networking;

/// <summary>
/// Sends magic packets as IPv4 limited broadcasts (255.255.255.255), which is what
/// a display sitting on the same subnet listens for.
/// </summary>
public sealed class UdpWakeOnLanTransport : IWakeOnLanTransport, IDisposable
{
    readonly UdpClient _udp = new(AddressFamily.InterNetwork) { EnableBroadcast = true };

    public Task SendAsync(ReadOnlyMemory<byte> packet, int port, CancellationToken cancellationToken)
        => _udp.SendAsync(packet, new IPEndPoint(IPAddress.Broadcast, port), cancellationToken).AsTask();

    public void Dispose() => _udp.Dispose();
}
