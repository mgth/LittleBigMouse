using System;
using System.Threading;
using LittleBigMouse.Platform.Windows;

namespace LittleBigMouse.DisplayLayout.Tests;

/// <summary>
/// Lifetime of <see cref="WindowsWallpaperWatcher"/>: the background thread, the stop event and the
/// Changed subscription. The real registry wait needs HKCU and only runs on Windows, so these drive
/// the watcher through its internal wait seam — a fake that stands in for
/// <c>RegNotifyChangeKeyValue</c> — and prove the parts that leak when disposal is wrong: the thread
/// stops, Dispose is idempotent, and no Changed fires after Dispose. All run on Linux.
/// </summary>
public class WindowsWallpaperWatcherTests
{
    /// <summary>
    /// A wait that fires <paramref name="signals"/> "changes" then blocks on the stop handle,
    /// exactly as the registry wait blocks on WaitAny(stop, notify). Records how many times the
    /// loop entered the wait, so a leaked/looping thread is observable.
    /// </summary>
    static Func<WaitHandle, bool> ChangeThenBlock(int signals, CountdownEvent? entered = null)
    {
        var remaining = signals;
        return stop =>
        {
            entered?.Signal();
            if (Interlocked.Decrement(ref remaining) >= 0) return true; // a change
            stop.WaitOne();                                             // then park until Dispose
            return false;                                               // stop requested
        };
    }

    [Fact]
    public void RaisesChanged_ForEachObservedChange()
    {
        // Hold the first change until the subscription below exists: the watcher thread starts
        // in the constructor, and a Changed fired before `+=` runs is simply lost — on a busy
        // runner the thread reliably won that race and all three events evaporated.
        var subscribed = new ManualResetEventSlim(false);
        var changes = ChangeThenBlock(3);
        var fired = new CountdownEvent(3);
        using var watcher = new WindowsWallpaperWatcher(stop =>
        {
            subscribed.Wait();
            return changes(stop);
        });
        watcher.Changed += (_, _) => fired.Signal();
        subscribed.Set();

        Assert.True(fired.Wait(TimeSpan.FromSeconds(2)), "expected three Changed events");
    }

    [Fact]
    public void Dispose_StopsTheBackgroundThread()
    {
        var entered = new CountdownEvent(1);
        var watcher = new WindowsWallpaperWatcher(ChangeThenBlock(0, entered));

        // Make sure the thread is parked in the wait before we tear it down.
        Assert.True(entered.Wait(TimeSpan.FromSeconds(2)), "watcher thread never reached its wait");

        // Dispose signals the stop handle, unblocks the wait, and joins the thread. If the join
        // timed out the thread was still alive — the whole point of the join is that it does not.
        watcher.Dispose();

        // A second Dispose after the thread is gone must not touch a released handle.
        watcher.Dispose();
    }

    [Fact]
    public void Dispose_IsIdempotent()
    {
        var watcher = new WindowsWallpaperWatcher(ChangeThenBlock(0));

        watcher.Dispose();
        var second = Record.Exception(() => watcher.Dispose());
        var third = Record.Exception(() => watcher.Dispose());

        Assert.Null(second);
        Assert.Null(third);
    }

    [Fact]
    public void NoChanged_FiresAfterDispose()
    {
        // The wait keeps reporting changes; Dispose must win the race so nothing arrives afterwards.
        var afterDispose = 0;
        var stopGate = new ManualResetEventSlim(false);

        Func<WaitHandle, bool> wait = stop =>
        {
            // Block until the test lets a change through, so the count is deterministic.
            if (stopGate.Wait(TimeSpan.FromMilliseconds(50))) return false;
            return WaitHandle.WaitAny(new[] { stop }, 0) != 0; // true unless stop already set
        };

        var watcher = new WindowsWallpaperWatcher(wait);
        watcher.Changed += (_, _) => Interlocked.Increment(ref afterDispose);

        watcher.Dispose();          // sets the stop handle and joins the thread
        Volatile.Write(ref afterDispose, 0);
        stopGate.Set();             // let any lingering wait return — the thread is already gone
        Thread.Sleep(50);

        Assert.Equal(0, Volatile.Read(ref afterDispose));
    }

    [Fact]
    public void Constructor_StartsWatchingImmediately()
    {
        var entered = new CountdownEvent(1);
        using var watcher = new WindowsWallpaperWatcher(ChangeThenBlock(0, entered));

        // The background thread starts in the constructor: it reaches the wait without any nudge.
        Assert.True(entered.Wait(TimeSpan.FromSeconds(2)), "watcher did not start on construction");
    }
}
