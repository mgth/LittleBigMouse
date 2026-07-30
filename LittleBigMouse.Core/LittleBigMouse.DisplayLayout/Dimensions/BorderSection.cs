using HLab.Base.ReactiveUI;
using LittleBigMouse.Zoning;

namespace LittleBigMouse.DisplayLayout.Dimensions;

/// <summary>
/// A stretch of one monitor edge with its own resistances. See <see cref="IBorderSection"/>
/// for the coordinate convention.
/// </summary>
public class BorderSection : SavableReactiveModel, IBorderSection
{
    public double From { get; set => SetUnsavedValue(ref field, value); }

    public double To { get; set => SetUnsavedValue(ref field, value); }

    public double Move { get; set => SetUnsavedValue(ref field, value); }

    public bool MoveBlock { get; set => SetUnsavedValue(ref field, value); }

    public double Drag { get; set => SetUnsavedValue(ref field, value); }

    public bool DragBlock { get; set => SetUnsavedValue(ref field, value); }
}
