namespace LittleBigMouse.Plugin.Vcp.Networking;

/// <summary>
/// Narrow network boundary used by Wake-on-LAN. The shipped implementation
/// broadcasts over UDP; tests can record the packets instead of sending them.
/// </summary>
public interface IWakeOnLanTransport
{
    Task SendAsync(ReadOnlyMemory<byte> packet, int port, CancellationToken cancellationToken);
}

/// <summary>
/// Shape of a magic packet burst. Displays disagree on how many packets they need
/// and how fast they accept them, so each caller states its own values instead of
/// the sender branching on the brand.
/// </summary>
public sealed record WakeOnLanOptions
{
    /// <summary>Number of identical magic packets sent in one burst.</summary>
    public int PacketCount { get; init; } = 3;

    /// <summary>Destination UDP port; 9 (discard) and 7 (echo) are the usual choices.</summary>
    public int Port { get; init; } = 9;

    /// <summary>Pause between two consecutive packets of the burst.</summary>
    public TimeSpan DelayBetweenPackets { get; init; } = TimeSpan.FromMilliseconds(100);

    /// <summary>
    /// When true the burst also waits <see cref="DelayBetweenPackets"/> after its last
    /// packet, so that awaiting the send means the display has had time to react.
    /// </summary>
    public bool DelayAfterLastPacket { get; init; }
}
