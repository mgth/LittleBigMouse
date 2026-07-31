using System;
using LittleBigMouse.DisplayLayout.Dimensions;

namespace LittleBigMouse.Plugin.Layout.Avalonia.BorderResistancePlugin;

/// <summary>
/// Which section is being edited, keyed by the MODEL rather than by a view model.
/// <para>
/// The same section is shown by several view models at once — the layout map and
/// the band on the real screen — so a selection held by one of them highlights
/// nothing in the others, and clicking a section on a screen selected nothing at
/// all, its band having no editor above it in the visual tree.
/// </para>
/// </summary>
public static class BorderSectionSelection
{
    public static BorderSection? Current { get; private set; }

    public static event Action<BorderSection?>? Changed;

    public static void Select(BorderSection? section)
    {
        if (ReferenceEquals(Current, section)) return;

        Current = section;
        Changed?.Invoke(section);
    }
}
