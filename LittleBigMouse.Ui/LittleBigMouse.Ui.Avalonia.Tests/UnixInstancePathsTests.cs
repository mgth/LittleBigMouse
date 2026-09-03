using Xunit;

namespace LittleBigMouse.Ui.Avalonia.Tests;

/// <summary>
/// The Unix guard's paths. A unix domain socket path is capped at 108 bytes, and an
/// XDG_RUNTIME_DIR long enough to break that used to crash the app out of Main with an
/// unhandled ArgumentOutOfRangeException, before any window existed. The lock is the guard
/// and stays where it is; the socket is the courtesy and may move, or go.
/// </summary>
public sealed class UnixInstancePathsTests : IDisposable
{
    readonly string _root = Path.Combine(Path.GetTempPath(), "lbm-guard-tests", Guid.NewGuid().ToString("N"));

    public UnixInstancePathsTests() => Directory.CreateDirectory(_root);

    public void Dispose()
    {
        try { Directory.Delete(_root, true); } catch { }
    }

    /// <summary>An existing directory whose path is at least that many characters long.</summary>
    string DirOfLength(int length)
    {
        var dir = Path.Combine(_root, new string('x', Math.Max(1, length - _root.Length - 1)));
        Directory.CreateDirectory(dir);
        return dir;
    }

    [Fact]
    public void ShortRuntimeDir_HoldsBoth()
    {
        var runtime = DirOfLength(60);

        var (lockPath, socketPath) = UnixInstancePaths.Resolve(runtime, _root, "me");

        Assert.Equal(Path.Combine(runtime, "littlebigmouse.lock"), lockPath);
        Assert.Equal(Path.Combine(runtime, "littlebigmouse-show.sock"), socketPath);
    }

    [Fact]
    public void LongRuntimeDir_KeepsTheLock_MovesTheSocketToAPrivateTempDir()
    {
        var runtime = DirOfLength(120);

        var (lockPath, socketPath) = UnixInstancePaths.Resolve(runtime, _root, "me");

        Assert.Equal(Path.Combine(runtime, "littlebigmouse.lock"), lockPath);
        Assert.Equal(Path.Combine(_root, "littlebigmouse-me", "littlebigmouse-show.sock"), socketPath);
        Assert.True(UnixInstancePaths.Fits(socketPath!));
    }

    [Fact]
    public void NoDirShortEnough_DropsTheSocket_NotTheLock()
    {
        var runtime = DirOfLength(120);
        var temp = DirOfLength(130);

        var (lockPath, socketPath) = UnixInstancePaths.Resolve(runtime, temp, "me");

        Assert.Equal(Path.Combine(runtime, "littlebigmouse.lock"), lockPath);
        Assert.Null(socketPath);
    }

    [Fact]
    public void MissingRuntimeDir_FallsBackToTemp()
    {
        var expectedLock = Path.Combine(_root, "littlebigmouse.lock");

        Assert.Equal(expectedLock, UnixInstancePaths.Resolve(null, _root, "me").LockPath);
        Assert.Equal(expectedLock, UnixInstancePaths.Resolve("", _root, "me").LockPath);
        Assert.Equal(expectedLock, UnixInstancePaths.Resolve(Path.Combine(_root, "missing"), _root, "me").LockPath);
        Assert.Equal(Path.Combine(_root, "littlebigmouse-show.sock"), UnixInstancePaths.Resolve(null, _root, "me").SocketPath);
    }

    [Fact]
    public void Fits_IsMeasuredInBytes()
    {
        Assert.True(UnixInstancePaths.Fits(new string('a', 107)));
        Assert.False(UnixInstancePaths.Fits(new string('a', 108)));
        // 60 characters, 120 bytes.
        Assert.False(UnixInstancePaths.Fits(new string('é', 60)));
    }

    [Fact]
    public void LongRuntimeDir_StillAcquiresTheInstance()
    {
        // The crash seen while setting up the #589 verification: TryAcquire threw out of Main.
        // Now: a guard, a lock that refuses a second acquire, a socket in the fallback dir.
        if (OperatingSystem.IsWindows()) return;

        var runtime = DirOfLength(120);
        var fallbackSocket = Path.Combine(_root, $"littlebigmouse-{Environment.UserName}", "littlebigmouse-show.sock");

        var guard = SingleInstanceGuard.TryAcquireUnix(runtime, _root);
        Assert.NotNull(guard);
        try
        {
            Assert.Null(SingleInstanceGuard.TryAcquireUnix(runtime, _root));
            Assert.True(File.Exists(fallbackSocket));
        }
        finally
        {
            guard!.Dispose();
        }

        Assert.False(File.Exists(Path.Combine(runtime, "littlebigmouse.lock")));
        Assert.False(File.Exists(fallbackSocket));
    }

    [Fact]
    public void NoSocketAtAll_StillAcquiresTheInstance()
    {
        if (OperatingSystem.IsWindows()) return;

        var runtime = DirOfLength(120);
        var temp = DirOfLength(130);

        var guard = SingleInstanceGuard.TryAcquireUnix(runtime, temp);
        Assert.NotNull(guard);
        try
        {
            Assert.Null(SingleInstanceGuard.TryAcquireUnix(runtime, temp));
        }
        finally
        {
            guard!.Dispose();
        }

        // After release a fresh acquire succeeds again: the lock really came back.
        var again = SingleInstanceGuard.TryAcquireUnix(runtime, temp);
        Assert.NotNull(again);
        again!.Dispose();
    }
}
