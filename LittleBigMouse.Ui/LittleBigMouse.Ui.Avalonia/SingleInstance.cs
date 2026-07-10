#nullable enable
using System;
using System.IO;
using System.Net.Sockets;
using System.Runtime.Versioning;
using System.Threading;

namespace LittleBigMouse.Ui.Avalonia;

/// <summary>
/// Cross-platform single-instance enforcement. <see cref="TryAcquire"/> returns null when
/// another instance already runs — after signaling it to show its window. The running
/// instance receives that signal through <see cref="ShowRequested"/> (raised on a background
/// thread; marshal to the UI thread before touching views).
/// Windows: named Mutex + named EventWaitHandle (unchanged historical behavior).
/// Linux: exclusive lock file + unix domain socket, both in XDG_RUNTIME_DIR.
/// </summary>
internal abstract class SingleInstanceGuard : IDisposable
{
    public event Action? ShowRequested;

    protected void RaiseShowRequested() => ShowRequested?.Invoke();

    public static SingleInstanceGuard? TryAcquire()
        => OperatingSystem.IsWindows()
            ? WindowsSingleInstanceGuard.TryAcquire()
            : UnixSingleInstanceGuard.TryAcquire();

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
        _wait.Unregister(null);
        _showEvent.Dispose();
        _mutex.ReleaseMutex();
        _mutex.Dispose();
    }
}

[UnsupportedOSPlatform("windows")]
file sealed class UnixSingleInstanceGuard : SingleInstanceGuard
{
    readonly FileStream _lock;
    readonly Socket _listener;
    readonly string _socketPath;

    UnixSingleInstanceGuard(FileStream fileLock, Socket listener, string socketPath)
    {
        _lock = fileLock;
        _listener = listener;
        _socketPath = socketPath;

        var thread = new Thread(AcceptLoop) { IsBackground = true, Name = "SingleInstance" };
        thread.Start();
    }

    static string RuntimeDir
        => Environment.GetEnvironmentVariable("XDG_RUNTIME_DIR") is { Length: > 0 } dir && Directory.Exists(dir)
            ? dir
            : Path.GetTempPath();

    public static new SingleInstanceGuard? TryAcquire()
    {
        var lockPath = Path.Combine(RuntimeDir, "littlebigmouse.lock");
        var socketPath = Path.Combine(RuntimeDir, "littlebigmouse-show.sock");

        FileStream fileLock;
        try
        {
            fileLock = new FileStream(lockPath, FileMode.OpenOrCreate, FileAccess.ReadWrite, FileShare.None);
        }
        catch (IOException)
        {
            // Another instance holds the lock: ask it to show its window, then exit.
            try
            {
                using var client = new Socket(AddressFamily.Unix, SocketType.Stream, ProtocolType.Unspecified);
                client.Connect(new UnixDomainSocketEndPoint(socketPath));
            }
            catch { }
            return null;
        }

        // We own the instance: a leftover socket file from a crashed run would fail the bind.
        File.Delete(socketPath);

        var listener = new Socket(AddressFamily.Unix, SocketType.Stream, ProtocolType.Unspecified);
        listener.Bind(new UnixDomainSocketEndPoint(socketPath));
        listener.Listen(1);

        return new UnixSingleInstanceGuard(fileLock, listener, socketPath);
    }

    void AcceptLoop()
    {
        while (true)
        {
            try
            {
                using var client = _listener.Accept();
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
        _listener.Dispose();
        try { File.Delete(_socketPath); } catch { }
        _lock.Dispose();
        try { File.Delete(_lock.Name); } catch { }
    }
}
