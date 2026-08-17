using System.Threading;
using Xunit;

namespace LittleBigMouse.Plugin.Vcp.Avalonia.Tests;

public class CalibrationTaskCoordinatorTests
{
    [Fact]
    public async Task RejectsASecondCalibrationWhileOneIsRunning()
    {
        using var coordinator = new CalibrationTaskCoordinator((_, _) => { });
        var entered = NewSignal();
        var release = NewSignal();
        var calls = 0;

        var first = coordinator.RunAsync("first", async token =>
        {
            Interlocked.Increment(ref calls);
            entered.SetResult();
            await release.Task.WaitAsync(token);
        });

        await entered.Task.WaitAsync(TimeSpan.FromSeconds(2));
        var second = await coordinator.RunAsync("second", _ =>
        {
            Interlocked.Increment(ref calls);
            return Task.CompletedTask;
        });

        Assert.False(second);
        Assert.Equal(1, Volatile.Read(ref calls));

        release.SetResult();
        Assert.True(await first.WaitAsync(TimeSpan.FromSeconds(2)));
    }

    [Fact]
    public async Task DisposeCancelsTheActiveCalibrationBeforeRunningCleanup()
    {
        var coordinator = new CalibrationTaskCoordinator((_, _) => { });
        var entered = NewSignal();
        var cancelled = NewSignal();
        var cleanup = NewSignal();

        var operation = coordinator.RunAsync("cancel", async token =>
        {
            entered.SetResult();
            try
            {
                await Task.Delay(Timeout.InfiniteTimeSpan, token);
            }
            finally
            {
                if (token.IsCancellationRequested) cancelled.SetResult();
            }
        });

        await entered.Task.WaitAsync(TimeSpan.FromSeconds(2));
        coordinator.Dispose(() => cleanup.SetResult());

        Assert.True(await operation.WaitAsync(TimeSpan.FromSeconds(2)));
        await cancelled.Task.WaitAsync(TimeSpan.FromSeconds(2));
        await cleanup.Task.WaitAsync(TimeSpan.FromSeconds(2));
    }

    [Fact]
    public async Task ReportsAndPropagatesCalibrationErrors()
    {
        string? reportedOperation = null;
        Exception? reportedError = null;
        using var coordinator = new CalibrationTaskCoordinator((operation, error) =>
        {
            reportedOperation = operation;
            reportedError = error;
        });
        var expected = new InvalidOperationException("probe failed");

        var actual = await Assert.ThrowsAsync<InvalidOperationException>(
            () => coordinator.RunAsync("brightness", _ => Task.FromException(expected)));

        Assert.Same(expected, actual);
        Assert.Equal("brightness", reportedOperation);
        Assert.Same(expected, reportedError);
    }

    static TaskCompletionSource NewSignal() =>
        new(TaskCreationOptions.RunContinuationsAsynchronously);
}
