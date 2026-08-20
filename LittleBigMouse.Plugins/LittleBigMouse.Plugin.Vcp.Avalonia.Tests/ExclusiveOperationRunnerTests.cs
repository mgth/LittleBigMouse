using Xunit;

namespace LittleBigMouse.Plugin.Vcp.Avalonia.Tests;

public class ExclusiveOperationRunnerTests
{
    const string TimeoutMessage = "The display did not answer in time.";

    static ExclusiveOperationRunner Runner(
        List<bool>? busy = null,
        List<string>? status = null,
        TimeSpan? defaultTimeout = null)
        => new(
            value => busy?.Add(value),
            message => status?.Add(message),
            TimeoutMessage,
            defaultTimeout ?? TimeSpan.FromSeconds(30));

    [Fact]
    public async Task DropsACommandStartedWhileAnotherIsInFlight()
    {
        var runner = Runner();
        var release = new TaskCompletionSource();
        var started = 0;

        var first = runner.RunAsync(async _ =>
        {
            started++;
            await release.Task;
        });
        await runner.RunAsync(_ =>
        {
            started++;
            return Task.CompletedTask;
        });

        Assert.Equal(1, started);
        Assert.True(runner.Busy);

        release.SetResult();
        await first;

        Assert.Equal(1, started);
        Assert.False(runner.Busy);
    }

    [Fact]
    public async Task AcceptsTheNextCommandOnceTheFirstOneIsDone()
    {
        var runner = Runner();
        var started = 0;

        await runner.RunAsync(_ => { started++; return Task.CompletedTask; });
        await runner.RunAsync(_ => { started++; return Task.CompletedTask; });

        Assert.Equal(2, started);
    }

    [Fact]
    public async Task RaisesAndClearsBusyAroundTheCommand()
    {
        var busy = new List<bool>();
        var runner = Runner(busy);

        await runner.RunAsync(_ => Task.CompletedTask);

        Assert.Equal([true, false], busy);
    }

    [Fact]
    public async Task TurnsAFailureIntoAStatusLineRatherThanLettingItEscape()
    {
        var busy = new List<bool>();
        var status = new List<string>();
        var runner = Runner(busy, status);

        await runner.RunAsync(_ => throw new InvalidOperationException("Associate a display first."));

        Assert.Equal(["Associate a display first."], status);
        Assert.Equal([true, false], busy);
        Assert.False(runner.Busy);
    }

    [Fact]
    public async Task ReportsTheTimeoutMessageWhenACommandOutlivesItsDeadline()
    {
        var status = new List<string>();
        var runner = Runner(status: status);

        await runner.RunAsync(
            token => Task.Delay(Timeout.Infinite, token),
            TimeSpan.FromMilliseconds(20));

        Assert.Equal([TimeoutMessage], status);
        Assert.False(runner.Busy);
    }

    [Fact]
    public async Task FallsBackToItsOwnDeadlineWhenTheCommandDoesNotAskForOne()
    {
        var status = new List<string>();
        var runner = Runner(status: status, defaultTimeout: TimeSpan.FromMilliseconds(20));

        await runner.RunAsync(token => Task.Delay(Timeout.Infinite, token));

        Assert.Equal([TimeoutMessage], status);
    }

    [Fact]
    public async Task LeavesTheDeadlineToTheCommandThatAsksForItsOwn()
    {
        var status = new List<string>();
        var runner = Runner(status: status, defaultTimeout: TimeSpan.FromMilliseconds(20));

        await runner.RunAsync(
            async _ => await Task.Delay(TimeSpan.FromMilliseconds(120)),
            TimeSpan.FromSeconds(30));

        Assert.Empty(status);
    }

    [Fact]
    public async Task ReportsATimeoutRatherThanAnEmptyMessageWhenTheCommandCancelsItself()
    {
        var status = new List<string>();
        var runner = Runner(status: status);

        await runner.RunAsync(_ => Task.FromCanceled(new CancellationToken(canceled: true)));

        Assert.Equal([TimeoutMessage], status);
    }
}
