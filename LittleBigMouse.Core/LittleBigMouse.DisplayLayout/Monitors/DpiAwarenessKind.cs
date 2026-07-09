namespace LittleBigMouse.DisplayLayout.Monitors;

/// <summary>
/// Platform-neutral mirror of the Windows DPI-awareness of the running UI process.
/// Values match the historical <c>WinDef.DpiAwareness</c> so the DIP-ratio formulas
/// in <see cref="PhysicalSource.UpdateDipToPixelRatio"/> are unchanged.
///
/// This is a <b>process-scoped</b> value (how the UI process/thread is manifested),
/// not a hardware property: the platform layout factory captures it on the UI side and
/// stamps it onto the layout. It lives with the model because the model is the neutral
/// abstraction — there is no separate abstraction layer.
/// </summary>
public enum DpiAwarenessKind
{
    Invalid = -1,
    Unaware = 0,
    SystemAware = 1,
    PerMonitorAware = 2
}
