using System.Buffers.Binary;
using MsBox.Avalonia.Enums;
using LittleBigMouse.Ui.Avalonia.Main;
using LittleBigMouse.Ui.Avalonia.MonitorFrame;
using LittleBigMouse.Ui.Avalonia.Remote;
using Xunit;

namespace LittleBigMouse.Ui.Avalonia.Tests;

public sealed class UiLifecycleTests
{
    [Theory]
    [InlineData(ButtonResult.Yes, true)]
    [InlineData(ButtonResult.No, false)]
    [InlineData(ButtonResult.None, false)]
    public void CloseConfirmationOnlyAcceptsYes(ButtonResult result, bool expected)
        => Assert.Equal(expected, MainViewModel.ShouldShutdown(result));

    [Fact]
    public void OlderAsyncResourceCannotReplaceNewerOne()
    {
        using var slot = new LatestResourceSlot<TestResource>();
        var olderGeneration = slot.Begin();
        var newerGeneration = slot.Begin();
        var newer = new TestResource();
        var older = new TestResource();

        Assert.True(slot.TryReplace(newerGeneration, newer));
        Assert.False(slot.TryReplace(olderGeneration, older));
        Assert.True(older.Disposed);
        Assert.False(newer.Disposed);
    }

    [Fact]
    public void ReplacingAndDisposingResourceReleasesEveryOwnedInstance()
    {
        var slot = new LatestResourceSlot<TestResource>();
        var first = new TestResource();
        var second = new TestResource();
        Assert.True(slot.TryReplace(slot.Begin(), first));
        Assert.True(slot.TryReplace(slot.Begin(), second));

        Assert.True(first.Disposed);
        Assert.False(second.Disposed);
        slot.Dispose();
        Assert.True(second.Disposed);
    }

    [Fact]
    public async Task MissingDaemonRaisesConnectionFailedInsteadOfWaitingForever()
    {
        // Windows-only: NamedPipeClientStream waits forever on a missing server,
        // which is the failure mode this pins down. On Linux the UDS path is
        // machine-global, so a developer's live daemon would make it flaky.
        if (!OperatingSystem.IsWindows()) return;

        using var client = new LocalIpcClient($"LittleBigMouse-test-{Guid.NewGuid():N}");
        var failed = new TaskCompletionSource(
            TaskCreationOptions.RunContinuationsAsynchronously);
        client.ConnectionFailed += (_, _) => failed.TrySetResult();

        client.Listen();

        await failed.Task.WaitAsync(TimeSpan.FromSeconds(2));
    }

    [Fact]
    public async Task CorruptUtf8FrameSurfacesAsInvalidDataNotDecoderFallback()
    {
        // The listener's catch filter reconnects on InvalidDataException; an
        // unlisted DecoderFallbackException would kill it silently forever.
        var payload = new byte[] { 0xFF, 0xFE, 0xFD };
        var frame = new byte[sizeof(uint) + payload.Length];
        BinaryPrimitives.WriteUInt32LittleEndian(frame, (uint)payload.Length);
        payload.CopyTo(frame, sizeof(uint));

        using var stream = new MemoryStream(frame);
        await Assert.ThrowsAsync<InvalidDataException>(
            () => LocalIpcClient.ReadFrameAsync(stream, CancellationToken.None));
    }

    [Theory]
    [InlineData(@"\\.\pipe\lbm-test", "lbm-test")]
    [InlineData("lbm-test", "lbm-test")]
    public void EndpointOverrideAcceptsFullPipePathOrBareName(string endpoint, string expected)
        => Assert.Equal(expected, LocalIpcClient.PipeNameFromEndpoint(endpoint));

    sealed class TestResource : IDisposable
    {
        public bool Disposed { get; private set; }
        public void Dispose() => Disposed = true;
    }
}
