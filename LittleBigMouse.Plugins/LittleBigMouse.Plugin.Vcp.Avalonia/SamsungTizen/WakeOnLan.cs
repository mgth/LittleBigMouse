#nullable enable
using System.Net;
using System.Net.Sockets;

namespace LittleBigMouse.Plugin.Vcp.Avalonia.SamsungTizen;

public static class WakeOnLan
{
    public static byte[] CreateMagicPacket(string macAddress)
    {
        var compact = new string(macAddress.Where(Uri.IsHexDigit).ToArray());
        if (compact.Length != 12) throw new FormatException("The MAC address must contain 12 hexadecimal digits.");

        var mac = Convert.FromHexString(compact);
        var packet = new byte[6 + 16 * mac.Length];
        Array.Fill(packet, (byte)0xff, 0, 6);
        for (var i = 6; i < packet.Length; i += mac.Length) mac.CopyTo(packet, i);
        return packet;
    }

    public static async Task SendAsync(string macAddress, CancellationToken cancellationToken = default)
    {
        var packet = CreateMagicPacket(macAddress);
        using var udp = new UdpClient(AddressFamily.InterNetwork) { EnableBroadcast = true };
        var destination = new IPEndPoint(IPAddress.Broadcast, 9);

        for (var attempt = 0; attempt < 3; attempt++)
        {
            cancellationToken.ThrowIfCancellationRequested();
            await udp.SendAsync(packet, destination, cancellationToken).ConfigureAwait(false);
            if (attempt < 2) await Task.Delay(100, cancellationToken).ConfigureAwait(false);
        }
    }
}

