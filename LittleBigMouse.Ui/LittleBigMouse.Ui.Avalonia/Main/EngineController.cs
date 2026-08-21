using System;
using System.Threading;
using System.Threading.Tasks;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.DisplayLayout.Monitors.Extensions;
using LittleBigMouse.Plugins;
using LittleBigMouse.Ui.Avalonia.Remote;
using LittleBigMouse.Zoning;

namespace LittleBigMouse.Ui.Avalonia.Main;

/// <summary>
/// How long the post-resume watchdog keeps re-asserting Start before giving up.
/// Defaults are the measured ones; tests shrink them.
/// </summary>
/// <param name="StepMs">Spacing between engine-state checks.</param>
/// <param name="StableSteps">Consecutive Running checks that count as converged.</param>
/// <param name="MaxRestarts">Cap on re-Starts, so a paused/excluded engine can't loop forever.</param>
/// <param name="MaxSteps">Hard stop against a pathological flapping wake.</param>
public sealed record ResumeWatchdogTimings(
    int StepMs = 500,
    int StableSteps = 3,
    int MaxRestarts = 6,
    int MaxSteps = 60)
{
    public static readonly ResumeWatchdogTimings Default = new();
}

/// <summary>
/// Owns the answer to one question: <em>should the mouse engine be hooked right now, and is
/// it?</em>
/// <para>
/// Starting is never simply "send Start". The daemon unhooks itself over any display change
/// so the cursor is never confined while the desktop reshapes, which means the UI is the side
/// that has to notice the hook went away and put it back — without ever forcing it on over a
/// user who turned it off, or over an exclusion the daemon is honouring.
/// </para>
/// <para>
/// The three ways back in, weakest first: <see cref="StartIfEnabledAsync"/> after a rebuild,
/// <see cref="EnsureHookedAsync"/> when a display change settled to nothing, and
/// <see cref="EnsureRunningAfterResumeAsync"/>, which is the only one that retries — a wake
/// from sleep is the one case where a single Start reliably loses a race.
/// </para>
/// </summary>
public sealed class EngineController(
    ILittleBigMouseClientService client,
    ILayoutPersistence persistence,
    Func<IMonitorsLayout?> currentLayout,
    ResumeWatchdogTimings? timings = null)
{
    readonly ResumeWatchdogTimings _timings = timings ?? ResumeWatchdogTimings.Default;

    /// <summary>
    /// Set while the display is off (daemon Suspended/Resumed events). Everything that would
    /// rebuild or re-hook stands down while it is set: there is no desktop to reason about, and
    /// the daemon has already unhooked itself. Written on the daemon-event thread — a single
    /// flag, so volatile is enough.
    /// </summary>
    public bool Suspended
    {
        get => _suspended;
        set => _suspended = value;
    }
    volatile bool _suspended;

    /// <summary>
    /// The daemon reports Running only while its low-level mouse hook is actually installed, so
    /// its published state is the source of truth for "is the engine hooked right now".
    /// </summary>
    public bool EngineRunning => client.State == LittleBigMouseEvent.Running;

    /// <summary>Has the user asked for the engine at all? Everything else is subordinate to this.</summary>
    public bool Enabled => currentLayout()?.Options.Enabled ?? false;

    /// <summary>True while <see cref="EnsureRunningAfterResumeAsync"/> owns reconciliation.</summary>
    public bool ResumeWatchdogActive => _resumeWatchdogActive;

    /// <summary>
    /// Hand the current layout to the daemon and hook. No-op before the first layout build —
    /// the boot sequence builds one before the tray exists, so this only guards the impossible.
    /// </summary>
    public Task StartAsync()
    {
        var layout = currentLayout();
        return layout is null ? Task.CompletedTask : client.StartAsync(layout.ComputeZones());
    }

    /// <summary>What a fresh layout generation does with itself: hook, if the user wants it hooked.</summary>
    public Task StartIfEnabledAsync() => Enabled ? StartAsync() : Task.CompletedTask;

    /// <summary>
    /// The user's Start — the tray entry and the window's apply button are the same gesture,
    /// and this is it. Persists Enabled=true first: the recovery guards all test
    /// <c>Options.Enabled</c>, so a start that doesn't record itself is one they will keep
    /// undoing.
    /// </summary>
    /// <param name="keepLayout">
    /// What the window's button adds over the tray's: "Apply and start" also keeps the geometry
    /// being edited, not merely the fact that the engine was asked for. The tray has no editor
    /// behind it — it hands over whatever the layout currently is and writes nothing else.
    /// </param>
    public async Task StartFromUserAsync(bool keepLayout = false)
    {
        if (currentLayout() is not { } layout) return;

        // A foreign layout is simulated, never adopted: the client sends Load without Run, so
        // the daemon validates the zones and reports on them with the input hook left down.
        // Enabled stays false — VirtualLayoutFactory sets it that way on purpose — so that no
        // recovery path (a rebuild, a re-hook, the resume watchdog) can mistake a foreign
        // geometry for something the user asked to run. Nothing is persisted either; the store
        // refuses a virtual layout anyway.
        if (layout.IsVirtual)
        {
            await StartAsync();
            return;
        }

        layout.Options.Enabled = true;
        await PersistStartAsync(layout, keepLayout);
        await StartAsync();
    }

    /// <summary>
    /// The user's Stop, symmetric to <see cref="StartFromUserAsync"/> — and with no virtual
    /// branch, on purpose: Enabled=false is the right value for a foreign layout anyway, and
    /// the store's own guard is what turns the write into a no-op. That guard exists for this
    /// exact call.
    /// </summary>
    public async Task StopFromUserAsync()
    {
        if (currentLayout() is { } layout)
        {
            layout.Options.Enabled = false;
            await Task.Run(() => persistence.SaveEnabled(layout));
        }

        await client.StopAsync();
    }

    /// <summary>
    /// Write the start down. Off the calling thread: both user Starts are clicks, and this
    /// reaches the store (registry, JSON file) and the autostart scheduling behind it.
    /// </summary>
    Task PersistStartAsync(IMonitorsLayout layout, bool keepLayout) =>
        Task.Run(() =>
        {
            // A full save writes Enabled too, so it stands in for the narrow one — but only an
            // edited layout is worth it, and only a MonitorsLayout can be saved whole at all.
            if (keepLayout && layout is MonitorsLayout { Saved: false } edited)
                persistence.Save(edited);
            else
                persistence.SaveEnabled(layout);
        });

    /// <summary>
    /// Re-hook if the engine should be running but is not — WITHOUT rebuilding the layout.
    /// Called when a display change settled back to the already-built configuration. Safe while
    /// an excluded app (a game) is focused: the daemon's Run path no-ops when paused, so this can
    /// never force the hook on over an exclusion.
    /// </summary>
    public async Task EnsureHookedAsync()
    {
        if (_resumeWatchdogActive) return; // the resume watchdog owns reconciliation
        if (_suspended) return;            // display off — nothing to do
        if (!Enabled) return;              // engine disabled by the user
        if (EngineRunning) return;         // already hooked

        await StartAsync();
    }

    /// <summary>
    /// Keep the engine hooked across the display re-enumeration storm that follows a wake from
    /// sleep. The daemon emits Resumed as soon as the screen turns on, but Windows then
    /// re-enumerates the GPU/monitors for several seconds, firing a burst of WM_DISPLAYCHANGE.
    /// One of those, processed just after we re-install the hook, makes the daemon unhook itself
    /// again (a Stopped ~1-2s after Resumed), and a single Start loses that race — the engine
    /// stays stopped ("blue") until a manual Start. Re-assert Start until it sticks (Running for
    /// a short quiet window), bounded so a legitimately-paused engine (excluded app focused at
    /// wake) can't loop forever.
    /// </summary>
    public async Task EnsureRunningAfterResumeAsync()
    {
        if (!Enabled) return;

        var myGen = Interlocked.Increment(ref _resumeGeneration);
        _resumeWatchdogActive = true;
        try
        {
            var restarts = 0;
            var quiet = 0;
            for (var i = 0; i < _timings.MaxSteps; i++)
            {
                if (myGen != Volatile.Read(ref _resumeGeneration)) return; // a newer resume superseded us
                if (_suspended) return;                                    // went back to sleep
                if (!Enabled) return;                                      // user disabled the engine

                if (EngineRunning)
                {
                    if (++quiet >= _timings.StableSteps) return; // Running long enough — converged
                }
                else if (restarts >= _timings.MaxRestarts)
                {
                    return; // gave up — the engine is legitimately paused (excluded app focused at wake)
                }
                else
                {
                    quiet = 0;
                    restarts++;
                    await StartAsync();
                }

                await Task.Delay(_timings.StepMs);
            }
        }
        finally
        {
            _resumeWatchdogActive = false;
        }
    }

    // A generation counter so a newer Resumed supersedes an older watchdog run, and a flag
    // telling EnsureHookedAsync to stand aside while the watchdog is the active driver. Written
    // on the daemon-event path — volatile is enough.
    int _resumeGeneration;
    volatile bool _resumeWatchdogActive;
}
