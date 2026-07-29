namespace LittleBigMouse.DisplayLayout.Monitors;

/// <summary>
/// Where a <see cref="IMonitorsLayout"/> comes from. Anything other than
/// <see cref="System"/> is a VIRTUAL layout: a foreign configuration displayed
/// for inspection. Virtual layouts must never reach the local store, the
/// autostart scheduling or the daemon's input hook — the guards live in the
/// persistence layer and in the daemon itself, all keyed on this value.
/// </summary>
public enum LayoutSource
{
    /// <summary>Built from the machine's actual displays.</summary>
    System,

    /// <summary>Loaded from an export file (LBM_VIRTUAL_LAYOUT).</summary>
    VirtualFile,

    /// <summary>Pasted or opened through the debug "open virtual layout" tool.</summary>
    VirtualImport,
}
