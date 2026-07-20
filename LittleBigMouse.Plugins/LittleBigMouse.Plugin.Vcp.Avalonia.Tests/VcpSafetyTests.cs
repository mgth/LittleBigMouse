using System.Collections.Generic;
using System.Threading;
using HLab.Sys.Windows.MonitorVcp;
using OneOf;
using Xunit;
using static HLab.Sys.Windows.API.MonitorConfiguration.LowLevelMonitorConfiguration;

namespace LittleBigMouse.Plugin.Vcp.Avalonia.Tests;

public class VcpSafetyTests
{
    [Fact]
    public void StableLevelReadsOnceAndDoesNotPollForever()
    {
        var reads = 0;
        var worker = new CommandWorker();
        using var level = new MonitorLevel(worker, _ =>
        {
            Interlocked.Increment(ref reads);
            return (50u, 0u, 100u);
        }, (_, _) => true).Start();

        Assert.True(SpinWait.SpinUntil(() => Volatile.Read(ref reads) == 1, 2_000));
        Thread.Sleep(150);

        Assert.Equal(1, Volatile.Read(ref reads));
        Assert.Equal(0, worker.PendingCount);
    }

    [Fact]
    public void UserChangePerformsOneWriteAndStopsAfterVerification()
    {
        uint remote = 50;
        var reads = 0;
        var writes = 0;
        var worker = new CommandWorker();
        using var level = new MonitorLevel(worker, _ =>
        {
            Interlocked.Increment(ref reads);
            return (remote, 0u, 100u);
        }, (value, _) =>
        {
            Interlocked.Increment(ref writes);
            remote = value;
            return true;
        }).Start();

        Assert.True(SpinWait.SpinUntil(() => Volatile.Read(ref reads) == 1, 2_000));
        level.Value = 65;
        Assert.True(SpinWait.SpinUntil(
            () => Volatile.Read(ref writes) == 1 && Volatile.Read(ref reads) >= 3, 2_000));
        var settledReads = Volatile.Read(ref reads);
        Thread.Sleep(150);

        Assert.Equal(1, Volatile.Read(ref writes));
        Assert.Equal(settledReads, Volatile.Read(ref reads));
        Assert.False(level.Moving);
    }

    [Fact]
    public void AmbiguousPhysicalMonitorsAreNotGuessed()
    {
        Assert.Null(DxVa2VcpTransport.SelectPhysicalMonitorIndex(
            ["Generic PnP Monitor", "Generic PnP Monitor"],
            ["Generic PnP Monitor", "DEL U2723QE"]));

        Assert.Equal(1, DxVa2VcpTransport.SelectPhysicalMonitorIndex(
            ["Left panel", "Dell U2723QE"],
            ["U2723QE"]));

        Assert.Equal(0, DxVa2VcpTransport.SelectPhysicalMonitorIndex(
            ["Unidentified monitor"], []));
    }

    [Fact]
    public void DisposingControlReleasesItsTransport()
    {
        var transport = new FakeTransport();
        var control = new VcpControl("monitor", "DEL", transport, new CommandWorker());

        control.Dispose();

        Assert.True(SpinWait.SpinUntil(() => transport.Disposed, 2_000));
    }

    sealed class FakeTransport : IVcpTransport
    {
        public bool Disposed { get; private set; }
        public IReadOnlySet<byte>? GetSupportedCodes() => new HashSet<byte>();
        public OneOf<(uint value, uint min, uint max), int> GetFeature(VcpCode code)
            => (0u, 0u, 100u);
        public bool SetFeature(VcpCode code, uint value) => true;
        public void Dispose() => Disposed = true;
    }
}
