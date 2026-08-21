using System.Buffers.Binary;
using System.Net.Sockets;
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
    public async Task ADaemonThatDisappearsAsksForARelaunch()
    {
        // The crash-recovery contract, and the reason ConnectionFailed exists at all.
        // Pinned here so the shutdown fix below cannot be made to work by silencing it.
        if (OperatingSystem.IsWindows()) return;

        using var daemon = new FakeDaemon();
        using var client = new LocalIpcClient(daemon.SocketPath);
        var failed = new TaskCompletionSource(
            TaskCreationOptions.RunContinuationsAsynchronously);
        var connected = new TaskCompletionSource(
            TaskCreationOptions.RunContinuationsAsynchronously);
        client.Connected += (_, _) => connected.TrySetResult();
        client.ConnectionFailed += (_, _) => failed.TrySetResult();

        client.Listen();
        await connected.Task.WaitAsync(TimeSpan.FromSeconds(5));

        daemon.Stop();

        await failed.Task.WaitAsync(TimeSpan.FromSeconds(5));
    }

    [Fact]
    public async Task AfterStopListeningAVanishingDaemonIsNotRelaunched()
    {
        // The orphan: a UI Exit sends Quit, the daemon closes the socket on its way
        // out, and the listener read that as a daemon to bring back — spawning the
        // `lbm-hook` that then outlived the UI. StopListening is what closes that door.
        if (OperatingSystem.IsWindows()) return;

        using var daemon = new FakeDaemon();
        using var client = new LocalIpcClient(daemon.SocketPath);
        var connected = new TaskCompletionSource(
            TaskCreationOptions.RunContinuationsAsynchronously);
        var relaunches = 0;
        client.Connected += (_, _) => connected.TrySetResult();
        client.ConnectionFailed += (_, _) => Interlocked.Increment(ref relaunches);

        client.Listen();
        await connected.Task.WaitAsync(TimeSpan.FromSeconds(5));

        client.StopListening();
        daemon.Stop();

        // Comfortably past the listener's ~350 ms reconnect cycle.
        await Task.Delay(TimeSpan.FromSeconds(2));
        Assert.Equal(0, Volatile.Read(ref relaunches));
    }

    /// <summary>
    /// A Unix socket that accepts and then says nothing — enough for the client to
    /// report Connected and send its Listen frame. <see cref="Stop"/> is the daemon
    /// exiting: the socket file is left behind, exactly as `lbm-hook` leaves it.
    /// </summary>
    sealed class FakeDaemon : IDisposable
    {
        readonly Socket _listener;
        readonly List<Socket> _accepted = [];

        public FakeDaemon()
        {
            // Well under SUN_LEN (108): a long temp path fails to bind at all.
            SocketPath = Path.Combine(Path.GetTempPath(), $"lbm-{Guid.NewGuid():N}.sock");
            _listener = new Socket(AddressFamily.Unix, SocketType.Stream, ProtocolType.Unspecified);
            _listener.Bind(new UnixDomainSocketEndPoint(SocketPath));
            _listener.Listen(4);
            _ = Task.Run(AcceptAsync);
        }

        public string SocketPath { get; }

        async Task AcceptAsync()
        {
            try
            {
                while (true)
                {
                    var accepted = await _listener.AcceptAsync();
                    lock (_accepted) _accepted.Add(accepted);
                }
            }
            catch (Exception error) when (error is SocketException or ObjectDisposedException)
            {
                // Stopped.
            }
        }

        public void Stop()
        {
            _listener.Dispose();
            lock (_accepted)
            {
                foreach (var accepted in _accepted) accepted.Dispose();
                _accepted.Clear();
            }
        }

        public void Dispose()
        {
            Stop();
            try { File.Delete(SocketPath); } catch (IOException) { }
        }
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
