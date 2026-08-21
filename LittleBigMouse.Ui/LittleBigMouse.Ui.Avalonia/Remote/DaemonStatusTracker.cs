using System;
using System.Threading.Tasks;
using LittleBigMouse.Zoning;
using ReactiveUI;

namespace LittleBigMouse.Ui.Avalonia.Remote;

/// <summary>
/// What the daemon says about itself, reduced to the three things the UI binds to: whether the
/// engine is hooked (<see cref="Running"/>), whether it is gone altogether (<see cref="Dead"/>),
/// and what it made of the last layout it was handed (<see cref="LayoutInfo"/>).
/// <para>
/// This is a translation, not a policy. It owns no command, no layout and no persistence: it turns
/// a <see cref="LittleBigMouseEvent"/> into state, and that is all. The one event carrying a
/// decision — <see cref="LittleBigMouseEvent.Rescued"/> — is handed straight back out through
/// <c>onRescued</c>, because what a rescue *means* depends on whether the user was previewing,
/// which is the view model's business.
/// </para>
/// <para>
/// Events that this build does not know about are ignored rather than rejected: a newer daemon
/// sending Suspended, Resumed or Probed must not fault the handler that also carries Running.
/// </para>
/// </summary>
/// <param name="onUiThread">
/// Runs a state write on the UI thread. <see cref="Apply"/> is called from the daemon's receive
/// thread, so every write to the properties below goes through here; the rest of this class is
/// only ever touched from the UI thread and does not marshal.
/// </param>
/// <param name="previewing">Whether a live preview is currently running — read only on a rescue.</param>
/// <param name="onRescued">
/// The panic shortcut ran. The daemon has freed the cursor and is coming down; that is all it can
/// decide knowing nothing, and it deliberately knows nothing, because it must work with no UI
/// reachable at all. The argument says whether a preview was interrupted.
/// </param>
public sealed class DaemonStatusTracker(
    Action<Action> onUiThread,
    Func<bool> previewing,
    Action<bool> onRescued) : ReactiveObject
{
    /// <summary>The daemon reports Running only while its low-level mouse hook is installed.</summary>
    public bool Running
    {
        get => _running;
        private set => this.RaiseAndSetIfChanged(ref _running, value);
    }
    bool _running;

    /// <summary>No daemon to talk to at all — distinct from one that is merely stopped.</summary>
    public bool Dead
    {
        get => _dead;
        private set => this.RaiseAndSetIfChanged(ref _dead, value);
    }
    bool _dead;

    /// <summary>
    /// Last Load outcome reported by the daemon ("3 zones (3 main), virtual" / a failure
    /// message). This is the only feedback a Load-without-Run produces — the virtual
    /// layout badge displays it as the simulation status.
    /// </summary>
    public string LayoutInfo
    {
        get => _layoutInfo;
        private set => this.RaiseAndSetIfChanged(ref _layoutInfo, value);
    }
    string _layoutInfo = "";

    /// <summary>
    /// Drop the last Load outcome: it belongs to the layout generation that just went away.
    /// Called from the UI thread, unlike <see cref="Apply"/>.
    /// </summary>
    public void ForgetLayoutInfo() => LayoutInfo = "";

    /// <summary>
    /// Fold one daemon event into the state above. Safe to call from the receive thread.
    /// </summary>
    public void Apply(LittleBigMouseServiceEventArgs e)
    {
        try
        {
            switch (e.Event)
            {
                case LittleBigMouseEvent.Running:
                    onUiThread(() =>
                    {
                        Dead = false;
                        Running = true;
                    });
                    break;

                case LittleBigMouseEvent.Stopped:
                    onUiThread(() =>
                    {
                        Dead = false;
                        Running = false;
                    });
                    break;

                case LittleBigMouseEvent.Dead:
                    onUiThread(() =>
                    {
                        Dead = true;
                        Running = false;
                    });
                    break;

                case LittleBigMouseEvent.Loaded:
                    onUiThread(() => LayoutInfo = e.Payload);
                    break;

                case LittleBigMouseEvent.LoadFailed:
                    onUiThread(() =>
                        LayoutInfo = string.IsNullOrEmpty(e.Payload) ? "load failed" : e.Payload);
                    break;

                // Previewing? Then there is an experiment to throw away: drop it, go back to the
                // saved layout and start again. Not previewing? Then what trapped the user is what
                // they committed to, and leaving the engine down is the right answer — no second
                // press. Either way the preview must end here, or the pump would hand the daemon
                // back, within its next tick, the very geometry the user just escaped from.
                case LittleBigMouseEvent.Rescued:
                    onUiThread(() =>
                    {
                        var wasPreviewing = previewing();
                        LayoutInfo = wasPreviewing
                            ? "rescued: back to the saved layout"
                            : "rescued: engine stopped";
                        onRescued(wasPreviewing);
                    });
                    break;

                case LittleBigMouseEvent.SettingsChanged:
                case LittleBigMouseEvent.DisplayChanged:
                case LittleBigMouseEvent.DesktopChanged:
                case LittleBigMouseEvent.FocusChanged:
                case LittleBigMouseEvent.Paused:
                case LittleBigMouseEvent.Connected:
                    break;

                // Anything else — including events a newer daemon knows about and this
                // build does not — is not ours to react to. This used to throw, which
                // made every Suspended, Resumed and Probed fault the handler.
                default:
                    break;
            }
        }
        catch (TaskCanceledException)
        {
        }
    }
}
