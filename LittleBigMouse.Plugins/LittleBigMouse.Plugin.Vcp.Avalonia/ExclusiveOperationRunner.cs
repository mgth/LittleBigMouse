#nullable enable

namespace LittleBigMouse.Plugin.Vcp.Avalonia;

/// <summary>
/// Runs one panel command at a time, under a deadline, and reports what happened as a single
/// status line.
///
/// The three rules are the same for every network-attached display, and none of them is
/// optional. A second command started while the first is in flight is dropped rather than
/// queued: the buttons it comes from are disabled meanwhile, so a command that gets through is
/// a double click or a key repeat, and the user wants it ignored, not replayed later. Every
/// command gets a deadline because a display that is asleep, moved to another address or simply
/// unplugged answers nothing at all — without one the panel would stay disabled for good. And a
/// failure becomes text in the panel instead of an exception, because these commands run from
/// <c>ReactiveCommand</c>, where an escaping exception takes the whole application down.
///
/// Expected to be used from the UI thread, as its callbacks write bound properties.
/// </summary>
/// <param name="reportBusy">Drives the flag the panel binds its buttons to.</param>
/// <param name="reportStatus">Shows a timeout or a failure to the user.</param>
/// <param name="timeoutMessage">
/// What to show when the deadline expires. Names the device and what to check, since this is
/// the failure users actually hit.
/// </param>
/// <param name="defaultTimeout">Deadline for the commands that do not ask for their own.</param>
public sealed class ExclusiveOperationRunner(
    Action<bool> reportBusy,
    Action<string> reportStatus,
    string timeoutMessage,
    TimeSpan defaultTimeout)
{
    /// <summary>True while a command is in flight, i.e. while further commands are dropped.</summary>
    public bool Busy { get; private set; }

    public async Task RunAsync(Func<CancellationToken, Task> operation, TimeSpan? timeout = null)
    {
        ArgumentNullException.ThrowIfNull(operation);
        if (Busy) return;

        Busy = true;
        reportBusy(true);
        using var deadline = new CancellationTokenSource(timeout ?? defaultTimeout);
        try
        {
            await operation(deadline.Token);
        }
        catch (OperationCanceledException)
        {
            reportStatus(timeoutMessage);
        }
        catch (Exception exception)
        {
            reportStatus(exception.Message);
        }
        finally
        {
            Busy = false;
            reportBusy(false);
        }
    }
}
