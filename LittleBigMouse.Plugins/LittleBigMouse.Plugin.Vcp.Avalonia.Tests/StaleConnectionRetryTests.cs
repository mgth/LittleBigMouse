using Xunit;

namespace LittleBigMouse.Plugin.Vcp.Avalonia.Tests;

public class StaleConnectionRetryTests
{
    static bool IsTransportFailure(Exception exception) => exception is IOException;

    /// <summary>Counts the attempts and fails the first <c>failures</c> of them like a dead socket.</summary>
    sealed class Transport(int failures)
    {
        public int Sends { get; private set; }
        public int Reopens { get; private set; }

        public Task SendAsync(CancellationToken cancellationToken)
        {
            cancellationToken.ThrowIfCancellationRequested();
            Sends++;
            return Sends <= failures
                ? Task.FromException(new IOException($"write {Sends} found the connection closed"))
                : Task.CompletedTask;
        }

        public Task ReopenAsync(CancellationToken cancellationToken)
        {
            Reopens++;
            return Task.CompletedTask;
        }
    }

    [Fact]
    public async Task SendsOnceAndLeavesTheSessionAloneWhenItIsAlive()
    {
        var transport = new Transport(failures: 0);

        await StaleConnectionRetry.SendAsync(transport.SendAsync, transport.ReopenAsync, IsTransportFailure);

        Assert.Equal(1, transport.Sends);
        Assert.Equal(0, transport.Reopens);
    }

    [Fact]
    public async Task ReopensAndSendsAgainWhenTheFirstWriteFindsTheSessionDead()
    {
        var transport = new Transport(failures: 1);

        await StaleConnectionRetry.SendAsync(transport.SendAsync, transport.ReopenAsync, IsTransportFailure);

        Assert.Equal(2, transport.Sends);
        Assert.Equal(1, transport.Reopens);
    }

    [Fact]
    public async Task GivesUpAfterOneRetryAndReportsTheSecondFailure()
    {
        var transport = new Transport(failures: 2);

        var error = await Assert.ThrowsAsync<IOException>(() => StaleConnectionRetry.SendAsync(
            transport.SendAsync, transport.ReopenAsync, IsTransportFailure));

        Assert.Equal("write 2 found the connection closed", error.Message);
        Assert.Equal(2, transport.Sends);
        Assert.Equal(1, transport.Reopens);
    }

    [Fact]
    public async Task DoesNotRepeatACommandTheDisplayRefused()
    {
        var sends = 0;
        var reopens = 0;

        await Assert.ThrowsAsync<UnauthorizedAccessException>(() => StaleConnectionRetry.SendAsync(
            _ =>
            {
                sends++;
                return Task.FromException(new UnauthorizedAccessException("the display denied IP control"));
            },
            _ => { reopens++; return Task.CompletedTask; },
            IsTransportFailure));

        Assert.Equal(1, sends);
        Assert.Equal(0, reopens);
    }

    [Fact]
    public async Task DoesNotMistakeACancelledCommandForADeadSession()
    {
        var transport = new Transport(failures: 0);
        using var cancellation = new CancellationTokenSource();
        await cancellation.CancelAsync();

        await Assert.ThrowsAnyAsync<OperationCanceledException>(() => StaleConnectionRetry.SendAsync(
            transport.SendAsync, transport.ReopenAsync, IsTransportFailure, cancellation.Token));

        Assert.Equal(0, transport.Sends);
        Assert.Equal(0, transport.Reopens);
    }

    [Fact]
    public async Task ReportsAFailureToReopenRatherThanSendingIntoTheVoid()
    {
        var transport = new Transport(failures: 1);

        var error = await Assert.ThrowsAsync<InvalidOperationException>(() => StaleConnectionRetry.SendAsync(
            transport.SendAsync,
            _ => Task.FromException(new InvalidOperationException("Pair this device first.")),
            IsTransportFailure));

        Assert.Equal("Pair this device first.", error.Message);
        Assert.Equal(1, transport.Sends);
    }

    [Fact]
    public async Task HandsTheCallerTokenToEveryAttemptAndToTheReopen()
    {
        using var cancellation = new CancellationTokenSource();
        var seen = new List<CancellationToken>();
        var sends = 0;

        await StaleConnectionRetry.SendAsync(
            token =>
            {
                seen.Add(token);
                return ++sends == 1 ? Task.FromException(new IOException("stale")) : Task.CompletedTask;
            },
            token => { seen.Add(token); return Task.CompletedTask; },
            IsTransportFailure,
            cancellation.Token);

        Assert.Equal(3, seen.Count);
        Assert.All(seen, token => Assert.Equal(cancellation.Token, token));
    }
}
