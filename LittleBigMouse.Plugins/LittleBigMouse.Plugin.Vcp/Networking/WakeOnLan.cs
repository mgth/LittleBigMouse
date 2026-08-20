using System.Net;

namespace LittleBigMouse.Plugin.Vcp.Networking;

/// <summary>
/// Standard Wake-on-LAN magic packets: six 0xff bytes followed by the target MAC
/// address repeated sixteen times.
/// </summary>
public static class WakeOnLan
{
    const int MacLength = 6;

    /// <summary>Length of every magic packet: 6 synchronisation bytes + 16 copies of the MAC.</summary>
    public const int MagicPacketLength = MacLength + 16 * MacLength;

    /// <summary>
    /// Reads the six address bytes from the usual notations: "aa:bb:cc:dd:ee:ff",
    /// "AA-BB-CC-DD-EE-FF", "aabb.ccdd.eeff" or twelve bare hexadecimal digits.
    /// </summary>
    /// <exception cref="FormatException">The value is not twelve hexadecimal digits.</exception>
    public static byte[] ParseMacAddress(string macAddress)
    {
        ArgumentNullException.ThrowIfNull(macAddress);

        Span<char> digits = stackalloc char[2 * MacLength];
        var count = 0;
        foreach (var character in macAddress)
        {
            if (character is ':' or '-' or '.' || char.IsWhiteSpace(character)) continue;
            if (count == digits.Length || !Uri.IsHexDigit(character)) throw NotAMacAddress(macAddress);
            digits[count++] = character;
        }
        if (count != digits.Length) throw NotAMacAddress(macAddress);

        return Convert.FromHexString(digits);
    }

    /// <summary>Builds the magic packet waking <paramref name="macAddress"/>.</summary>
    public static byte[] CreateMagicPacket(string macAddress)
    {
        var mac = ParseMacAddress(macAddress);

        var packet = new byte[MagicPacketLength];
        Array.Fill(packet, (byte)0xff, 0, MacLength);
        for (var offset = MacLength; offset < packet.Length; offset += MacLength) mac.CopyTo(packet, offset);
        return packet;
    }

    /// <summary>
    /// Broadcasts one burst of magic packets for <paramref name="macAddress"/>.
    /// </summary>
    /// <param name="transport">Defaults to a UDP broadcast socket owned by this call.</param>
    /// <param name="timeProvider">Defaults to the system clock; tests can shorten the burst.</param>
    public static async Task SendAsync(
        string macAddress,
        WakeOnLanOptions options,
        IWakeOnLanTransport? transport = null,
        TimeProvider? timeProvider = null,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(options);
        ArgumentOutOfRangeException.ThrowIfLessThan(options.PacketCount, 1);
        ArgumentOutOfRangeException.ThrowIfLessThan(options.Port, IPEndPoint.MinPort);
        ArgumentOutOfRangeException.ThrowIfGreaterThan(options.Port, IPEndPoint.MaxPort);
        if (options.DelayBetweenPackets < TimeSpan.Zero)
            throw new ArgumentOutOfRangeException(nameof(options), options.DelayBetweenPackets,
                "The delay between packets cannot be negative.");

        var packet = CreateMagicPacket(macAddress);
        var time = timeProvider ?? TimeProvider.System;

        using var ownedTransport = transport is null ? new UdpWakeOnLanTransport() : null;
        var target = transport ?? ownedTransport!;

        for (var sent = 0; sent < options.PacketCount; sent++)
        {
            cancellationToken.ThrowIfCancellationRequested();
            await target.SendAsync(packet, options.Port, cancellationToken).ConfigureAwait(false);

            var isLastPacket = sent == options.PacketCount - 1;
            if (isLastPacket && !options.DelayAfterLastPacket) break;
            if (options.DelayBetweenPackets > TimeSpan.Zero)
                await Task.Delay(options.DelayBetweenPackets, time, cancellationToken).ConfigureAwait(false);
        }
    }

    static FormatException NotAMacAddress(string value) => new(
        $"'{value}' is not a MAC address: expected twelve hexadecimal digits, "
        + "optionally grouped by ':', '-' or '.'.");
}
