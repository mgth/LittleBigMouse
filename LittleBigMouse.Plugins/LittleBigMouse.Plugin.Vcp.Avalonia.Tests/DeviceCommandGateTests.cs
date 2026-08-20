using Xunit;

namespace LittleBigMouse.Plugin.Vcp.Avalonia.Tests;

public class DeviceCommandGateTests
{
    [Fact]
    public async Task HoldsTheSecondCommandUntilTheFirstOneIsDone()
    {
        var gate = new DeviceCommandGate();
        var release = new TaskCompletionSource();
        var order = new List<string>();

        var first = gate.RunExclusiveAsync(async _ =>
        {
            order.Add("first started");
            await release.Task;
            order.Add("first finished");
        });
        var second = gate.RunExclusiveAsync(_ =>
        {
            order.Add("second started");
            return Task.CompletedTask;
        });

        Assert.False(second.IsCompleted);
        Assert.Equal(["first started"], order);

        release.SetResult();
        await Task.WhenAll(first, second);

        Assert.Equal(["first started", "first finished", "second started"], order);
    }

    [Fact]
    public async Task ReturnsWhatTheCommandProduced()
        => Assert.Equal("token", await new DeviceCommandGate().RunExclusiveAsync(_ => Task.FromResult("token")));

    [Fact]
    public async Task LetsTheNextCommandThroughWhenOneFails()
    {
        var gate = new DeviceCommandGate();

        await Assert.ThrowsAsync<InvalidOperationException>(() => gate.RunExclusiveAsync(async _ =>
        {
            await Task.Yield();
            throw new InvalidOperationException("the display closed the channel");
        }));

        Assert.True(await gate.RunExclusiveAsync(_ => Task.FromResult(true)));
    }

    [Fact]
    public async Task GivesUpWaitingWhenTheCallerCancelsAndStaysUsable()
    {
        var gate = new DeviceCommandGate();
        var release = new TaskCompletionSource();
        var ran = false;

        var blocking = gate.RunExclusiveAsync(_ => release.Task);
        using var cancellation = new CancellationTokenSource();
        var queued = gate.RunExclusiveAsync(_ =>
        {
            ran = true;
            return Task.CompletedTask;
        }, cancellation.Token);

        await cancellation.CancelAsync();
        await Assert.ThrowsAnyAsync<OperationCanceledException>(() => queued);
        Assert.False(ran);

        release.SetResult();
        await blocking;
        Assert.True(await gate.RunExclusiveAsync(_ => Task.FromResult(true)));
    }

    [Fact]
    public async Task ClosingWaitsForTheCommandInFlightBeforeTearingDownTheConnection()
    {
        var gate = new DeviceCommandGate();
        var release = new TaskCompletionSource();
        var order = new List<string>();

        var command = gate.RunExclusiveAsync(async _ =>
        {
            await release.Task;
            order.Add("command finished");
        });
        var closing = gate.CloseAsync(() =>
        {
            order.Add("connection closed");
            return Task.CompletedTask;
        });

        Assert.False(closing.IsCompleted);

        release.SetResult();
        await Task.WhenAll(command, closing);

        Assert.Equal(["command finished", "connection closed"], order);
    }

    [Fact]
    public async Task ClosedGateRefusesFurtherCommands()
    {
        var gate = new DeviceCommandGate();
        await gate.CloseAsync(() => Task.CompletedTask);

        await Assert.ThrowsAsync<ObjectDisposedException>(() => gate.RunExclusiveAsync(_ => Task.CompletedTask));
        await Assert.ThrowsAsync<ObjectDisposedException>(() => gate.RunExclusiveAsync(_ => Task.FromResult(1)));
    }

    [Fact]
    public async Task ClosingTwiceTearsTheConnectionDownOnce()
    {
        var gate = new DeviceCommandGate();
        var closes = 0;

        await gate.CloseAsync(() => { closes++; return Task.CompletedTask; });
        await gate.CloseAsync(() => { closes++; return Task.CompletedTask; });

        Assert.Equal(1, closes);
    }
}
