#nullable enable
using LittleBigMouse.DisplayLayout;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Plugins;

namespace LittleBigMouse.Platform.Windows;

/// <summary>
/// Windows implementation of <see cref="ILayoutPersistence"/>: a thin adapter over the
/// historical registry extension methods (<see cref="PersistencyExtensions"/>), which stay
/// the real implementation — WindowsLayoutFactory keeps calling them directly within this
/// assembly. Registry values are byte-for-byte unchanged.
/// </summary>
public class WindowsLayoutPersistence : ILayoutPersistence
{
    public bool IsLoading => PersistencyExtensions.IsLoading;

    public void Load(MonitorsLayout layout) => layout.Load();

    public bool Save(MonitorsLayout layout) => layout.Save();

    public bool SaveEnabled(IMonitorsLayout layout) => layout.SaveEnabled();

    public void SaveLive(ILayoutOptions options) => options.SaveLive();
}
