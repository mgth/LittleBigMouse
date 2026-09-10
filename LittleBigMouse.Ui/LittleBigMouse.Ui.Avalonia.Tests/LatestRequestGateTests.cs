using System.Collections.Generic;
using System.Threading.Tasks;
using LittleBigMouse.Ui.Avalonia.Remote;
using Xunit;

namespace LittleBigMouse.Ui.Avalonia.Tests;

public class LatestRequestGateTests
{
    [Fact]
    public async Task OnlyTheNewestRequestLands()
    {
        var gate = new LatestRequestGate();
        var sent = new List<string>();

        var older = gate.Claim();
        var newer = gate.Claim();

        Assert.False(await gate.SendIfLatestAsync(older, () => { sent.Add("older"); return Task.CompletedTask; }));
        Assert.True(await gate.SendIfLatestAsync(newer, () => { sent.Add("newer"); return Task.CompletedTask; }));

        Assert.Equal(new[] { "newer" }, sent);
    }

    [Fact]
    public async Task AStartAskedForBeforeARebuildIsDroppedWhenItsTurnComesAfter()
    {
        // #607 as a sequence: the Start of the old layout is claimed first but reaches the
        // gate second — behind the Start of the rebuilt layout, which was claimed later.
        var gate = new LatestRequestGate();
        var sent = new List<string>();
        var holdNewer = new TaskCompletionSource();

        var oldLayout = gate.Claim();
        var newLayout = gate.Claim();

        var newerSend = gate.SendIfLatestAsync(newLayout, async () => { await holdNewer.Task; sent.Add("new layout"); });
        var olderSend = gate.SendIfLatestAsync(oldLayout, () => { sent.Add("old layout"); return Task.CompletedTask; });

        holdNewer.SetResult();

        Assert.True(await newerSend);
        Assert.False(await olderSend);
        Assert.Equal(new[] { "new layout" }, sent);
    }

    [Fact]
    public async Task AStopSupersedesAStartStillOnItsWayAndAlwaysLandsItself()
    {
        var gate = new LatestRequestGate();
        var sent = new List<string>();

        var start = gate.Claim();
        gate.Claim(); // the Stop
        await gate.SendAsync(() => { sent.Add("stop"); return Task.CompletedTask; });

        Assert.False(await gate.SendIfLatestAsync(start, () => { sent.Add("start"); return Task.CompletedTask; }));
        Assert.Equal(new[] { "stop" }, sent);
    }

    [Fact]
    public async Task ASendInProgressIsFollowedNeverOvertaken()
    {
        // A Start that already passed the check is on the wire; the Stop that came after
        // it queues behind it. Reversing them would leave the engine up after a Stop.
        var gate = new LatestRequestGate();
        var sent = new List<string>();
        var holdStart = new TaskCompletionSource();

        var start = gate.Claim();
        var startSend = gate.SendIfLatestAsync(start, async () => { await holdStart.Task; sent.Add("start"); });

        gate.Claim();
        var stopSend = gate.SendAsync(() => { sent.Add("stop"); return Task.CompletedTask; });
        Assert.False(stopSend.IsCompleted);
        Assert.Empty(sent);

        holdStart.SetResult();
        Assert.True(await startSend);
        await stopSend;

        Assert.Equal(new[] { "start", "stop" }, sent);
    }
}
