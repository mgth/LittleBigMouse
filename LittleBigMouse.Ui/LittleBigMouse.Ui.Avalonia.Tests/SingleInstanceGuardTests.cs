using System;
using LittleBigMouse.Ui.Avalonia;
using Xunit;

namespace LittleBigMouse.Ui.Avalonia.Tests;

/// <summary>
/// Disposal of the single-instance guard, exercised through the real <see cref="SingleInstanceGuard.TryAcquire"/>.
/// On Linux this is the Unix guard (lock file + domain socket); the Windows guard (Mutex +
/// EventWaitHandle + RegisteredWaitHandle) takes the same idempotent teardown path but its OS
/// primitives only exist on Windows, so only the Unix path is provable here. The property that
/// matters on both is that Dispose is idempotent: the Windows guard's ReleaseMutex would throw on a
/// second call, the Unix guard would delete a socket file a fresh instance may have just re-created.
/// </summary>
public sealed class SingleInstanceGuardTests
{
    [Fact]
    public void Acquire_ThenDisposeTwice_DoesNotThrow()
    {
        // Off Windows this acquires the Unix lock/socket in XDG_RUNTIME_DIR (or the temp dir).
        var guard = SingleInstanceGuard.TryAcquire();

        // If something already holds the instance in this environment there is nothing to dispose;
        // the double-dispose contract is only meaningful for a guard we actually own.
        if (guard is null) return;

        guard.Dispose();
        var second = Record.Exception(() => guard.Dispose());
        var third = Record.Exception(() => guard.Dispose());

        Assert.Null(second);
        Assert.Null(third);
    }

    [Fact]
    public void SecondAcquire_WhileFirstHeld_ReportsAlreadyRunning_ThenReleases()
    {
        // A named Mutex is re-entrant on the thread that owns it: a same-thread second acquire
        // succeeds instead of reporting "already running", so on Windows this contract is only
        // observable across processes. In-process it is provable on the Unix guard alone (the
        // FileShare.None lock refuses a second open regardless of who holds it).
        if (OperatingSystem.IsWindows()) return;

        var first = SingleInstanceGuard.TryAcquire();
        if (first is null) return; // another instance owns it; nothing to assert about our own

        try
        {
            // While the first guard holds the lock, a second acquire must report "already running"
            // (null) rather than acquire a duplicate — the whole point of the guard.
            var second = SingleInstanceGuard.TryAcquire();
            Assert.Null(second);
        }
        finally
        {
            first.Dispose();
        }

        // After release a fresh acquire succeeds again: the handles really came back.
        var third = SingleInstanceGuard.TryAcquire();
        Assert.NotNull(third);
        third!.Dispose();
    }
}
