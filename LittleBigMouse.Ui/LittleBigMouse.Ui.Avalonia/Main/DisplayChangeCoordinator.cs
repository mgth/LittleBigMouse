using System;
using System.Threading;
using System.Threading.Tasks;

namespace LittleBigMouse.Ui.Avalonia.Main;

/// <summary>
/// How long a display-change burst is allowed to take before the layout is rebuilt.
/// Defaults are the measured ones; tests shrink them so a burst can be replayed in
/// milliseconds instead of seconds.
/// </summary>
/// <param name="DebounceMs">Quiet window absorbing the message burst.</param>
/// <param name="StabilityStepMs">Spacing between settle re-checks.</param>
/// <param name="StabilityMaxSteps">Cap on the settle loop, so a flapping config can't hang it.</param>
public sealed record DisplayChangeTimings(
    int DebounceMs = 300,
    int StabilityStepMs = 100,
    int StabilityMaxSteps = 8)
{
    public static readonly DisplayChangeTimings Default = new();
}

/// <summary>
/// Decides <em>when</em> a display change deserves a layout rebuild — and, just as often,
/// that it deserves none.
/// <para>
/// Attach/detach, resolution, scaling and primary changes all arrive as a burst of
/// WM_DISPLAYCHANGE/WM_SETTINGCHANGE (or, where no daemon reports them, as repeated
/// <see cref="LittleBigMouse.Plugins.ILayoutFactory.DisplayChanged"/> notifications), and the
/// OS is still moving while they arrive. Three filters, in order:
/// </para>
/// <list type="number">
/// <item>a trailing debounce — a generation counter, so only the newest event of a burst
/// proceeds and the rest collapse into it;</item>
/// <item>settle detection — rebuild only once two consecutive
/// <see cref="LittleBigMouse.Plugins.ILayoutFactory.DisplaySignature"/> reads agree, which on
/// real switches happens within one sample, so this reacts in a few hundred ms rather than
/// waiting out a fixed delay;</item>
/// <item>idempotence — a settled configuration identical to the one already built is not
/// rebuilt at all. A monitor's DPMS power-save cycle, a mode re-apply or a stray broadcast
/// settles back to the very same signature, and dropping a fully-wired layout generation that
/// Avalonia's compositor keeps alive is what fills gigabytes over a storm (#412).</item>
/// </list>
/// <para>
/// Everything it touches is behind delegates: no Avalonia, no daemon, no clock beyond
/// <see cref="Task.Delay(int)"/>.
/// </para>
/// </summary>
public sealed class DisplayChangeCoordinator
{
    readonly Func<string> _readSignature;
    readonly Func<bool> _isSuspended;
    readonly Action _rebuildLayout;
    readonly Func<Task> _startIfEnabledAsync;
    readonly Func<Task> _reconcileWithoutRebuildAsync;
    readonly DisplayChangeTimings _timings;

    /// <param name="readSignature">Cheap fingerprint of the current display configuration.</param>
    /// <param name="isSuspended">True while the display is off: nothing is read or rebuilt then.</param>
    /// <param name="rebuildLayout">Builds a fresh layout generation. Synchronous, like the model it drives.</param>
    /// <param name="startIfEnabledAsync">Hands the freshly built layout to the engine, if the user has it enabled.</param>
    /// <param name="reconcileWithoutRebuildAsync">
    /// Run when the configuration settled back to the already-built one. The daemon unhooks
    /// itself over <em>any</em> display change, so skipping the rebuild must not mean skipping
    /// the re-hook — nothing else would re-Start it and the engine would sit stopped ("blue")
    /// until a manual Start.
    /// </param>
    /// <param name="timings">Defaults to <see cref="DisplayChangeTimings.Default"/>.</param>
    public DisplayChangeCoordinator(
        Func<string> readSignature,
        Func<bool> isSuspended,
        Action rebuildLayout,
        Func<Task> startIfEnabledAsync,
        Func<Task> reconcileWithoutRebuildAsync,
        DisplayChangeTimings? timings = null)
    {
        _readSignature = readSignature;
        _isSuspended = isSuspended;
        _rebuildLayout = rebuildLayout;
        _startIfEnabledAsync = startIfEnabledAsync;
        _reconcileWithoutRebuildAsync = reconcileWithoutRebuildAsync;
        _timings = timings ?? DisplayChangeTimings.Default;
    }

    int _generation;

    /// <summary>
    /// Signature of the display configuration the current layout was built from. Empty until
    /// the first build, which is what makes the very first change always rebuild.
    /// </summary>
    public string LastBuiltSignature { get; private set; } = "";

    /// <summary>How many rebuilds have actually happened. Lets a burst be counted, not just observed.</summary>
    public int RebuildCount { get; private set; }

    /// <summary>
    /// A display change was reported. Returns once this particular notification has been
    /// resolved — by rebuilding, by reconciling, or by being superseded by a newer one.
    /// </summary>
    public async Task NotifyAsync()
    {
        // Nothing at all while the display is off: no debounce, no signature reads, no rebuild.
        // The daemon's Resumed event reconciles once when the desktop is back.
        if (_isSuspended()) return;

        var gen = Interlocked.Increment(ref _generation);

        // Trailing debounce: wait a short quiet window; if a newer event arrived meanwhile, bail
        // and let that later call handle it, so a burst collapses to a single rebuild.
        await Task.Delay(_timings.DebounceMs);
        if (gen != Volatile.Read(ref _generation)) return;

        // Settle detection: rebuild only once the display config has stopped changing, i.e. two
        // consecutive signatures match. Capped so a pathological flapping config can't hang here.
        var signature = _readSignature();
        for (var i = 0; i < _timings.StabilityMaxSteps; i++)
        {
            await Task.Delay(_timings.StabilityStepMs);
            if (gen != Volatile.Read(ref _generation)) return;
            var next = _readSignature();
            if (next == signature) break;
            signature = next;
        }

        // The display may have gone off during the debounce/settle: drop the rebuild — the daemon
        // has unhooked and we wait for Resumed.
        if (_isSuspended()) return;

        // The settle loop confirms the configuration STOPPED changing, not that it DIFFERS from
        // what we already built. Re-hook without rebuilding when it does not.
        if (signature == LastBuiltSignature)
        {
            await _reconcileWithoutRebuildAsync();
            return;
        }

        _rebuildLayout();
        RebuildCount++;
        LastBuiltSignature = signature;
        await _startIfEnabledAsync();
    }

    /// <summary>
    /// Force a rebuild the automatic detection missed (#443) — the tray's Refresh. Deliberately
    /// bypasses the debounce, the settle loop and the idempotence guard, then realigns the built
    /// signature so the next display event doesn't rebuild again on its account.
    /// </summary>
    public async Task RefreshAsync()
    {
        // Display off: nothing to rebuild against, the daemon's Resumed event reconciles.
        if (_isSuspended()) return;

        _rebuildLayout();
        RebuildCount++;
        LastBuiltSignature = _readSignature();
        await _startIfEnabledAsync();
    }
}
