using LittleBigMouse.Plugin.Vcp.Networking;
using Xunit;

namespace LittleBigMouse.Plugin.Vcp.Avalonia.Tests;

public class WakeOnLanTests
{
    static readonly byte[] Mac = [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff];

    [Fact]
    public void CreatesStandardMagicPacket()
    {
        var packet = WakeOnLan.CreateMagicPacket("AA:BB:CC:DD:EE:FF");

        Assert.Equal(102, packet.Length);
        Assert.All(packet.Take(6), value => Assert.Equal(0xff, value));
        for (var offset = 6; offset < packet.Length; offset += 6)
            Assert.Equal(Mac, packet.Skip(offset).Take(6));
    }

    [Theory]
    [InlineData("AA:BB:CC:DD:EE:FF")]
    [InlineData("aa:bb:cc:dd:ee:ff")]
    [InlineData("AA-BB-CC-DD-EE-FF")]
    [InlineData("aabb.ccdd.eeff")]
    [InlineData("aabbccddeeff")]
    [InlineData("AA BB CC DD EE FF")]
    [InlineData(" AA:BB:CC:DD:EE:FF ")]
    public void AcceptsTheUsualMacNotations(string macAddress)
        => Assert.Equal(Mac, WakeOnLan.ParseMacAddress(macAddress));

    [Theory]
    [InlineData("")]
    [InlineData("AA:BB:CC:DD:EE")]           // too short
    [InlineData("AA:BB:CC:DD:EE:FF:00")]     // too long
    [InlineData("AA:BB:CC:DD:EE:GG")]        // G is not hexadecimal
    [InlineData("AABBCCDDEEFFGG")]           // trailing junk, not silently dropped
    [InlineData("aa/bb/cc/dd/ee/ff")]        // unsupported separator
    public void RejectsMalformedMacAddresses(string macAddress)
        => Assert.Throws<FormatException>(() => WakeOnLan.ParseMacAddress(macAddress));

    [Fact]
    public async Task SendsOnePacketPerBurstEntryOnTheRequestedPort()
    {
        var transport = new RecordingTransport();
        var options = new WakeOnLanOptions { PacketCount = 4, Port = 7, DelayBetweenPackets = TimeSpan.Zero };

        await WakeOnLan.SendAsync("AA:BB:CC:DD:EE:FF", options, transport);

        Assert.Equal(4, transport.Sends.Count);
        Assert.All(transport.Sends, send => Assert.Equal(7, send.Port));
        Assert.All(transport.Sends, send => Assert.Equal(WakeOnLan.CreateMagicPacket("aabbccddeeff"), send.Packet));
    }

    [Fact]
    public async Task WaitsBetweenPacketsButNotAfterTheLastOne()
    {
        var transport = new RecordingTransport();
        var time = new RecordingTimeProvider();
        var options = new WakeOnLanOptions
        {
            PacketCount = 3,
            DelayBetweenPackets = TimeSpan.FromMilliseconds(100),
        };

        await WakeOnLan.SendAsync("AA:BB:CC:DD:EE:FF", options, transport, time);

        Assert.Equal(3, transport.Sends.Count);
        Assert.Equal([TimeSpan.FromMilliseconds(100), TimeSpan.FromMilliseconds(100)], time.Delays);
    }

    [Fact]
    public async Task WaitsAfterTheLastPacketWhenAsked()
    {
        var transport = new RecordingTransport();
        var time = new RecordingTimeProvider();
        var options = new WakeOnLanOptions
        {
            PacketCount = 3,
            DelayBetweenPackets = TimeSpan.FromMilliseconds(150),
            DelayAfterLastPacket = true,
        };

        await WakeOnLan.SendAsync("AA:BB:CC:DD:EE:FF", options, transport, time);

        Assert.Equal(3, transport.Sends.Count);
        Assert.Equal(3, time.Delays.Count);
        Assert.All(time.Delays, delay => Assert.Equal(TimeSpan.FromMilliseconds(150), delay));
    }

    [Fact]
    public async Task SkipsWaitingWhenTheBurstHasASinglePacket()
    {
        var transport = new RecordingTransport();
        var time = new RecordingTimeProvider();

        await WakeOnLan.SendAsync("AA:BB:CC:DD:EE:FF", new WakeOnLanOptions { PacketCount = 1 }, transport, time);

        Assert.Single(transport.Sends);
        Assert.Empty(time.Delays);
    }

    [Fact]
    public async Task RejectsAnEmptyBurst()
    {
        var options = new WakeOnLanOptions { PacketCount = 0 };

        await Assert.ThrowsAsync<ArgumentOutOfRangeException>(
            () => WakeOnLan.SendAsync("AA:BB:CC:DD:EE:FF", options, new RecordingTransport()));
    }

    [Fact]
    public async Task ValidatesTheMacAddressBeforeTouchingTheTransport()
    {
        var transport = new RecordingTransport();

        await Assert.ThrowsAsync<FormatException>(
            () => WakeOnLan.SendAsync("nope", new WakeOnLanOptions(), transport));
        Assert.Empty(transport.Sends);
    }

    [Fact]
    public async Task StopsOnCancellationBeforeSendingAnything()
    {
        var transport = new RecordingTransport();
        using var cancellation = new CancellationTokenSource();
        await cancellation.CancelAsync();

        await Assert.ThrowsAsync<OperationCanceledException>(
            () => WakeOnLan.SendAsync(
                "AA:BB:CC:DD:EE:FF", new WakeOnLanOptions(), transport, cancellationToken: cancellation.Token));
        Assert.Empty(transport.Sends);
    }

    sealed class RecordingTransport : IWakeOnLanTransport
    {
        public List<(byte[] Packet, int Port)> Sends { get; } = [];

        public Task SendAsync(ReadOnlyMemory<byte> packet, int port, CancellationToken cancellationToken)
        {
            cancellationToken.ThrowIfCancellationRequested();
            Sends.Add((packet.ToArray(), port));
            return Task.CompletedTask;
        }
    }

    /// <summary>
    /// Records what the sender asked to wait for and fires straight away, so the burst
    /// options can be asserted exactly without spending the delays.
    /// </summary>
    sealed class RecordingTimeProvider : TimeProvider
    {
        public List<TimeSpan> Delays { get; } = [];

        public override ITimer CreateTimer(TimerCallback callback, object? state, TimeSpan dueTime, TimeSpan period)
        {
            Delays.Add(dueTime);
            return new ImmediateTimer(callback, state);
        }

        sealed class ImmediateTimer : ITimer
        {
            public ImmediateTimer(TimerCallback callback, object? state)
                => Task.Run(() => callback(state));

            public bool Change(TimeSpan dueTime, TimeSpan period) => true;
            public void Dispose() { }
            public ValueTask DisposeAsync() => ValueTask.CompletedTask;
        }
    }
}
