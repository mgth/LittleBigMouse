using System.Net;
using System.Net.NetworkInformation;
using System.Net.Sockets;

namespace LittleBigMouse.Plugin.Vcp.Avalonia.HisenseVidaa;

static class WakeOnLan
{
    public static async Task SendAsync(string mac, CancellationToken ct)
    {
        var bytes = PhysicalAddress.Parse(mac.Replace(":", "").Replace("-", "")).GetAddressBytes();
        if (bytes.Length != 6) throw new ArgumentException("Invalid Wi-Fi MAC address.", nameof(mac));
        var packet = new byte[102]; Array.Fill(packet, (byte)0xFF, 0, 6);
        for (var i=6; i<packet.Length; i++) packet[i] = bytes[(i-6)%6];
        using var udp = new UdpClient(); udp.EnableBroadcast = true;
        for (var i=0; i<3; i++) { await udp.SendAsync(packet, new IPEndPoint(IPAddress.Broadcast, 9), ct); await Task.Delay(150, ct); }
    }
}
