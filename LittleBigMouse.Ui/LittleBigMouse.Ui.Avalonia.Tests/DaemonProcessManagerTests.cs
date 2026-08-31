using System;
using System.Collections.Generic;
using System.Linq;
using LittleBigMouse.Ui.Avalonia.Remote;
using Xunit;

namespace LittleBigMouse.Ui.Avalonia.Tests;

/// <summary>
/// The daemon <em>process</em> lifecycle, driven through a simulated process host and file seams so
/// no real daemon is ever spawned (spawning one would capture the machine's mice) and no test
/// touches the real per-user data directory. Covers the four things this manager is responsible
/// for: launching at most one, not launching over a live one, force-stopping only what it owns, and
/// releasing its handle on disposal without killing the daemon.
/// </summary>
public sealed class DaemonProcessManagerTests
{
    /// <summary>A simulated process. Never touches the OS.</summary>
    sealed class FakeProcess(string name, int id, int sessionId) : IDaemonProcess
    {
        public bool HasExited { get; set; }
        public int SessionId { get; } = sessionId;
        public int Id { get; } = id;
        public string ProcessName { get; } = name;

        public bool StopRefused { get; set; }
        public int StopCalls { get; private set; }
        public bool Disposed { get; private set; }

        public bool TryStop()
        {
            StopCalls++;
            if (StopRefused) return false;
            HasExited = true;
            return true;
        }

        public void Dispose() => Disposed = true;
    }

    sealed class FakeProcessHost : IProcessHost
    {
        public int CurrentSessionId { get; set; } = 1;

        /// <summary>Processes visible to enumeration.</summary>
        public List<FakeProcess> Running { get; } = [];

        /// <summary>Every handle enumeration has handed out — to assert they get disposed.</summary>
        public List<FakeProcess> Handed { get; } = [];

        public List<FakeProcess> Launched { get; } = [];
        public Func<string, FakeProcess>? LaunchFactory { get; set; }
        public Exception? LaunchThrows { get; set; }

        public IEnumerable<IDaemonProcess> Enumerate(IEnumerable<string> processNames)
        {
            var names = processNames.ToHashSet();
            // Snapshot so a TryStop that flips HasExited mid-iteration is fine.
            foreach (var process in Running.Where(p => names.Contains(p.ProcessName)).ToList())
            {
                Handed.Add(process);
                yield return process;
            }
        }

        public IDaemonProcess Launch(string path)
        {
            if (LaunchThrows is not null) throw LaunchThrows;
            var process = LaunchFactory?.Invoke(path)
                          ?? new FakeProcess("lbm-hook", 1000 + Launched.Count, CurrentSessionId);
            Launched.Add(process);
            Running.Add(process);
            return process;
        }
    }

    /// <summary>A manager wired to fakes: a fixed hook path and a no-op exclusion-file seed.</summary>
    static DaemonProcessManager Manager(FakeProcessHost host, string? hookPath = "/fake/lbm-hook")
        => new(host, seedExcludedFile: () => { }, resolveHookPath: () => hookPath);

    [Fact]
    public void ADaemonIsLaunchedWhenNoneIsRunning()
    {
        var host = new FakeProcessHost();
        var manager = Manager(host);

        manager.LaunchDaemon();

        Assert.Single(host.Launched);
    }

    [Fact]
    public void TheExclusionFileIsSeededBeforeLaunch()
    {
        var host = new FakeProcessHost();
        var seeded = false;
        var manager = new DaemonProcessManager(host,
            seedExcludedFile: () => seeded = true, resolveHookPath: () => "/fake/lbm-hook");

        manager.LaunchDaemon();

        Assert.True(seeded);
        Assert.Single(host.Launched);
    }

    [Fact]
    public void ASeedFailureDoesNotAbortTheLaunch()
    {
        // Seeding the exclusion file is best-effort; the daemon must still start.
        var host = new FakeProcessHost();
        var manager = new DaemonProcessManager(host,
            seedExcludedFile: () => throw new System.IO.IOException("disk full"),
            resolveHookPath: () => "/fake/lbm-hook");

        manager.LaunchDaemon();

        Assert.Single(host.Launched);
    }

    [Fact]
    public void NothingLaunchesWhenTheHookExecutableCannotBeFound()
    {
        var host = new FakeProcessHost();
        var manager = Manager(host, hookPath: null);

        manager.LaunchDaemon();

        Assert.Empty(host.Launched);
    }

    [Fact]
    public void ADaemonAlreadyRunningIsNotLaunchedAgain()
    {
        var host = new FakeProcessHost();
        host.Running.Add(new FakeProcess("lbm-hook", 42, host.CurrentSessionId));
        var manager = Manager(host);

        manager.LaunchDaemon();

        Assert.Empty(host.Launched);
    }

    [Fact]
    public void AnAlreadyRunningCheckDisposesEveryEnumeratedHandle()
    {
        // Enumerated handles are short-lived: the manager must dispose each one it inspects.
        var host = new FakeProcessHost();
        var live = new FakeProcess("lbm-hook", 42, host.CurrentSessionId);
        host.Running.Add(live);
        var manager = Manager(host);

        manager.LaunchDaemon();

        Assert.True(live.Disposed);
    }

    [Fact]
    public void AnExitedProcessDoesNotBlockARelaunch()
    {
        var host = new FakeProcessHost();
        host.Running.Add(new FakeProcess("lbm-hook", 42, host.CurrentSessionId) { HasExited = true });
        var manager = Manager(host);

        manager.LaunchDaemon();

        Assert.Single(host.Launched);
        Assert.All(host.Handed, p => Assert.True(p.Disposed));
    }

    [Fact]
    public void ReplacingTheLaunchedHandleDisposesThePreviousOne()
    {
        var host = new FakeProcessHost();
        var manager = Manager(host);

        manager.LaunchDaemon();
        var first = host.Launched.Single();
        // The first handle stays visible-but-exited so the second launch's guard lets it through.
        first.HasExited = true;

        manager.LaunchDaemon();

        Assert.Equal(2, host.Launched.Count);
        Assert.True(first.Disposed);
    }

    [Fact]
    public void OnUnixStopKillsOnlyTheLaunchedInstance()
    {
        if (OperatingSystem.IsWindows()) return; // Unix-only ownership rule.

        var host = new FakeProcessHost();
        var mine = new FakeProcess("lbm-hook", 1000, host.CurrentSessionId);
        host.LaunchFactory = _ => mine;
        var manager = Manager(host);
        manager.LaunchDaemon();

        // A foreign daemon this UI never launched appears afterwards — must be left alone: the
        // Unix path stops only the owned handle and never enumerates.
        var foreign = new FakeProcess("lbm-hook", 7, sessionId: 2);
        host.Running.Add(foreign);

        var stopped = manager.StopCurrentSessionDaemons();

        Assert.True(stopped);
        Assert.Equal(1, mine.StopCalls);
        Assert.Equal(0, foreign.StopCalls); // never touched
    }

    [Fact]
    public void OnUnixWithNothingLaunchedStopReportsClearOnlyWhenNoDaemonLingers()
    {
        if (OperatingSystem.IsWindows()) return;

        var host = new FakeProcessHost();
        var manager = Manager(host);

        // No launched instance and no lbm-hook in the table → nothing lingers → "stopped".
        Assert.True(manager.StopCurrentSessionDaemons());

        // A live foreign daemon means the field is not clear (but it is not ours to kill).
        host.Running.Add(new FakeProcess("lbm-hook", 7, 2));
        Assert.False(manager.StopCurrentSessionDaemons());
    }

    [Fact]
    public void StopReportsFailureWhenTheProcessRefusesToDie()
    {
        if (OperatingSystem.IsWindows()) return;

        var host = new FakeProcessHost();
        var mine = new FakeProcess("lbm-hook", 1000, host.CurrentSessionId) { StopRefused = true };
        host.LaunchFactory = _ => mine;
        var manager = Manager(host);
        manager.LaunchDaemon();

        Assert.False(manager.StopCurrentSessionDaemons());
        Assert.Equal(1, mine.StopCalls);
    }

    [Fact]
    public void DisposeReleasesTheOwnedHandleWithoutKillingTheDaemon()
    {
        var host = new FakeProcessHost();
        var mine = new FakeProcess("lbm-hook", 1000, host.CurrentSessionId);
        host.LaunchFactory = _ => mine;
        var manager = Manager(host);
        manager.LaunchDaemon();

        manager.Dispose();

        Assert.True(mine.Disposed);      // handle released
        Assert.Equal(0, mine.StopCalls); // but NOT force-stopped
        Assert.False(mine.HasExited);
    }
}
