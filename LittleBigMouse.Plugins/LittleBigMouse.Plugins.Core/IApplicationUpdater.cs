namespace LittleBigMouse.Plugins;

public interface IApplicationUpdater
{
    /// <summary>
    /// False when the app cannot update itself and updates come from elsewhere
    /// (a distribution package, for instance). Callers should hide any
    /// "check for update" affordance instead of showing a dead one.
    /// </summary>
    bool IsSupported { get; }

    Task CheckUpdateAsync(bool show);
}