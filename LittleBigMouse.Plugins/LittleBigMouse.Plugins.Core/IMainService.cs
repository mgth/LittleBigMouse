using LittleBigMouse.DisplayLayout.Monitors;

namespace LittleBigMouse.Plugins;

public interface IMainService
{
    void UpdateLayout();

    /// <summary>
    /// Rebuild the layout from the machine's actual displays, dropping any virtual
    /// layout currently shown — including one forced by LBM_VIRTUAL_LAYOUT, which is
    /// ignored from this call on (until the app restarts).
    /// </summary>
    void ReloadSystemLayout();

    /// <summary>
    /// Close the configuration window, which is how this process leaves (v6): the mouse
    /// engine is the agent's and keeps running. It goes through the window rather than
    /// through the application so that the unsaved-changes question is still asked — a
    /// shutdown would close over it.
    /// </summary>
    void CloseControl();

    IMonitorsLayout MonitorsLayout {get; set;}

    /// <summary>
    /// The layout being edited is also being fed to the mouse engine as it is edited.
    /// <para>
    /// Set by the control that owns the switch, read on the way out: it is what decides
    /// whether "Save" may be offered when leaving with an unsaved layout. Live update
    /// means the geometry is the one driving the mouse right now, so saving it saves
    /// something the user has felt — offering to save an untried one, in the hurry of a
    /// dialog standing between them and leaving, is how a bad layout becomes the one
    /// that loads at the next boot.
    /// </para>
    /// </summary>
    bool LivePreview {get; set;}

    Task StartNotifierAsync();

    Task ShowControlAsync();

    void AddControlPlugin(Action<IMainPluginsViewModel>? action);

}