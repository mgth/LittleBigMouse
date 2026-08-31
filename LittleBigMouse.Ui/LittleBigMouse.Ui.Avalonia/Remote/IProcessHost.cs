using System.Collections.Generic;

namespace LittleBigMouse.Ui.Avalonia.Remote;

/// <summary>
/// The operating-system process operations <see cref="DaemonProcessManager"/> needs, isolated
/// behind a seam so its ownership and stop/relaunch logic can be tested without spawning real
/// daemons. Production is <see cref="SystemProcessHost"/>, a thin wrapper over
/// <see cref="System.Diagnostics.Process"/>.
/// </summary>
public interface IProcessHost
{
    /// <summary>The session this UI runs in — used only on Windows to scope hook enumeration.</summary>
    int CurrentSessionId { get; }

    /// <summary>Every live-or-dead process currently matching one of the given image names.</summary>
    IEnumerable<IDaemonProcess> Enumerate(IEnumerable<string> processNames);

    /// <summary>
    /// Start the daemon executable at <paramref name="path"/>. Returns the owned handle. Throws on
    /// launch failure; the caller decides how loudly to fail.
    /// </summary>
    IDaemonProcess Launch(string path);
}

/// <summary>
/// A handle to a (possibly foreign) daemon process. Enumerated handles are owned by the caller and
/// must be disposed; the one returned by <see cref="IProcessHost.Launch"/> is owned by
/// <see cref="DaemonProcessManager"/> for the lifetime of the service.
/// </summary>
public interface IDaemonProcess : System.IDisposable
{
    bool HasExited { get; }
    int SessionId { get; }
    int Id { get; }
    string ProcessName { get; }

    /// <summary>Force-stop the process tree and wait briefly. True if it is gone afterwards.</summary>
    bool TryStop();
}
