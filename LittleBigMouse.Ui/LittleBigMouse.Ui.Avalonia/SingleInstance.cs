#nullable enable
using System;
using System.IO;
using System.Net.Sockets;
using System.Runtime.Versioning;
using System.Text;
using System.Threading;

namespace LittleBigMouse.Ui.Avalonia;

/// <summary>
/// Cross-platform single-instance enforcement. <see cref="TryAcquire"/> returns null when
/// another instance already runs — after signaling it to show its window. The running
/// instance receives that signal through <see cref="ShowRequested"/> (raised on a background
/// thread; marshal to the UI thread before touching views).
/// Windows: named Mutex + named EventWaitHandle (unchanged historical behavior).
/// Linux: exclusive lock file + unix domain socket in XDG_RUNTIME_DIR — see
/// <see cref="UnixInstancePaths"/> for where the socket goes when that path is too long for one.
/// </summary>
internal abstract class SingleInstanceGuard : IDisposable
{
    public event Action? ShowRequested;

    protected void RaiseShowRequested() => ShowRequested?.Invoke();

    public static SingleInstanceGuard? TryAcquire()
        => OperatingSystem.IsWindows()
            ? WindowsSingleInstanceGuard.TryAcquire()
            : UnixSingleInstanceGuard.TryAcquire();

    /// <summary>The Unix guard over explicit directories, for tests: no environment mutation.</summary>
    [UnsupportedOSPlatform("windows")]
    internal static SingleInstanceGuard? TryAcquireUnix(string? runtimeDir, string tempDir)
        => UnixSingleInstanceGuard.TryAcquire(runtimeDir, tempDir);

    public abstract void Dispose();
}

[SupportedOSPlatform("windows")]
file sealed class WindowsSingleInstanceGuard : SingleInstanceGuard
{
    const string APP_GUID = "51B5711E-1A7F-436E-B3DD-B598901B3FD2";
    const string SHOW_EVENT_NAME = APP_GUID + "_ShowWindow";

    readonly Mutex _mutex;
    readonly EventWaitHandle _showEvent;
    readonly RegisteredWaitHandle _wait;
    int _disposed;

    WindowsSingleInstanceGuard(Mutex mutex)
    {
        _mutex = mutex;
        _showEvent = new EventWaitHandle(false, EventResetMode.AutoReset, SHOW_EVENT_NAME);

        // RegisterWaitForSingleObject parks no thread of its own — a pool thread wakes only
        // when the event is signalled (AutoReset, so each new launch fires the callback once).
        _wait = ThreadPool.RegisterWaitForSingleObject(
            _showEvent,
            (_, _) => RaiseShowRequested(),
            state: null,
            millisecondsTimeOutInterval: Timeout.Infinite,
            executeOnlyOnce: false);
    }

    public static new SingleInstanceGuard? TryAcquire()
    {
        var mutex = new Mutex(true, APP_GUID);

        if (mutex.WaitOne(TimeSpan.Zero, false)) return new WindowsSingleInstanceGuard(mutex);

        // Signal the running instance to show its window, then report "already running".
        try
        {
            using var handle = EventWaitHandle.OpenExisting(SHOW_EVENT_NAME);
            handle.Set();
        }
        catch { }

        mutex.Dispose();
        return null;
    }

    public override void Dispose()
    {
        // Idempotent: a second call must not reach ReleaseMutex() again — the mutex would no longer
        // be owned and it would throw ApplicationException.
        if (Interlocked.Exchange(ref _disposed, 1) != 0) return;

        _wait.Unregister(null);
        _showEvent.Dispose();
        _mutex.ReleaseMutex();
        _mutex.Dispose();
    }
}

/// <summary>
/// Where the Unix guard puts its two files. The lock file can live anywhere; the socket
/// cannot: a unix domain socket path is capped at <see cref="MaxSocketPathBytes"/> bytes
/// (<c>sun_path</c>), and an XDG_RUNTIME_DIR long enough to break that — a sandbox, a test
/// runner, a nested session — used to take the whole app down with an unhandled
/// <see cref="ArgumentOutOfRangeException"/> out of Main, before anything was on screen.
/// <para>
/// The socket then falls back to a private per-user directory under the temp path, the
/// classic stand-in for a runtime dir. When even that is too long the guard runs without
/// it: still single-instance (the lock is the guard), only the "show your window" nudge to
/// the running instance is lost.
/// </para>
/// </summary>
internal static class UnixInstancePaths
{
    /// <summary>
    /// <c>sizeof(sun_path)</c> is 108 on Linux, terminator included. .NET checks the UTF-8
    /// length against 108; one less keeps the terminator out of the count everywhere.
    /// </summary>
    public const int MaxSocketPathBytes = 107;

    public const string LockFile = "littlebigmouse.lock";
    public const string SocketFile = "littlebigmouse-show.sock";

    /// <summary>The lock path, and the socket path when a short enough one exists.</summary>
    public static (string LockPath, string? SocketPath) Resolve(string? runtimeDir, string tempDir, string userName)
    {
        var baseDir = runtimeDir is { Length: > 0 } && Directory.Exists(runtimeDir) ? runtimeDir : tempDir;
        var lockPath = Path.Combine(baseDir, LockFile);

        var socketPath = Path.Combine(baseDir, SocketFile);
        if (Fits(socketPath)) return (lockPath, socketPath);

        var fallback = Path.Combine(tempDir, $"littlebigmouse-{userName}", SocketFile);
        return (lockPath, Fits(fallback) ? fallback : null);
    }

    public static bool Fits(string socketPath) => Encoding.UTF8.GetByteCount(socketPath) <= MaxSocketPathBytes;
}

[UnsupportedOSPlatform("windows")]
file sealed class UnixSingleInstanceGuard : SingleInstanceGuard
{
    readonly FileStream _lock;
    readonly Socket? _listener;
    readonly string? _socketPath;
    int _disposed;

    UnixSingleInstanceGuard(FileStream fileLock, Socket? listener, string? socketPath)
    {
        _lock = fileLock;
        _listener = listener;
        _socketPath = socketPath;

        if (listener is null) return;
        var thread = new Thread(AcceptLoop) { IsBackground = true, Name = "SingleInstance" };
        thread.Start();
    }

    public static new SingleInstanceGuard? TryAcquire()
        => TryAcquire(Environment.GetEnvironmentVariable("XDG_RUNTIME_DIR"), Path.GetTempPath());

    public static SingleInstanceGuard? TryAcquire(string? runtimeDir, string tempDir)
    {
        var (lockPath, socketPath) = UnixInstancePaths.Resolve(runtimeDir, tempDir, Environment.UserName);

        FileStream fileLock;
        try
        {
            fileLock = new FileStream(lockPath, FileMode.OpenOrCreate, FileAccess.ReadWrite, FileShare.None);
        }
        catch (IOException)
        {
            // Another instance holds the lock: ask it to show its window, then exit.
            if (socketPath is not null)
            {
                try
                {
                    using var client = new Socket(AddressFamily.Unix, SocketType.Stream, ProtocolType.Unspecified);
                    client.Connect(new UnixDomainSocketEndPoint(socketPath));
                }
                catch { }
            }
            return null;
        }

        return new UnixSingleInstanceGuard(fileLock, Listen(socketPath), socketPath);
    }

    /// <summary>
    /// The "show your window" listener, or null when no socket path is short enough or the
    /// bind fails. The lock already made this the instance; losing the nudge is not worth
    /// losing the app, which is what an exception here used to do.
    /// </summary>
    static Socket? Listen(string? socketPath)
    {
        if (socketPath is null)
        {
            Console.Error.WriteLine(
                "Single instance: no directory short enough for a unix socket; a second launch will not be able to show this window.");
            return null;
        }

        Socket? listener = null;
        try
        {
            // Only the fallback directory is ours to create. Private: anyone who can connect
            // can pop the window.
            var dir = Path.GetDirectoryName(socketPath)!;
            if (!Directory.Exists(dir))
                Directory.CreateDirectory(dir, UnixFileMode.UserRead | UnixFileMode.UserWrite | UnixFileMode.UserExecute);

            // We own the instance: a leftover socket file from a crashed run would fail the bind.
            File.Delete(socketPath);

            listener = new Socket(AddressFamily.Unix, SocketType.Stream, ProtocolType.Unspecified);
            listener.Bind(new UnixDomainSocketEndPoint(socketPath));
            listener.Listen(1);
            return listener;
        }
        catch (Exception error)
        {
            listener?.Dispose();
            Console.Error.WriteLine(
                $"Single instance: the show-window socket could not be set up at '{socketPath}'; a second launch will not be able to show this window: {error.Message}");
            return null;
        }
    }

    void AcceptLoop()
    {
        while (true)
        {
            try
            {
                using var client = _listener!.Accept();
                RaiseShowRequested();
            }
            catch (SocketException)
            {
                return; // listener disposed: shutting down
            }
            catch (ObjectDisposedException)
            {
                return;
            }
        }
    }

    public override void Dispose()
    {
        // Idempotent, to match the Windows guard: disposing the socket/lock twice is harmless, but
        // deleting the socket/lock files twice could race a fresh instance that just re-created them.
        if (Interlocked.Exchange(ref _disposed, 1) != 0) return;

        _listener?.Dispose();
        if (_socketPath is not null)
        {
            try { File.Delete(_socketPath); } catch { }
        }
        _lock.Dispose();
        try { File.Delete(_lock.Name); } catch { }
    }
}
