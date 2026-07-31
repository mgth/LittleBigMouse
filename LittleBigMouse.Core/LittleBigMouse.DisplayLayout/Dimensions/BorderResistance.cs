using HLab.Base.ReactiveUI;
using LittleBigMouse.Zoning;

namespace LittleBigMouse.DisplayLayout.Dimensions;

/// <summary>
/// The four edges of a monitor, each with its own move/drag resistances and
/// sections. This was four bare doubles before the move/drag split; a layout
/// saved in that shape migrates to <c>Move == Drag</c>, which is exactly what the
/// single value used to mean.
/// </summary>
public class BorderResistance : SavableReactiveModel, IBorderResistance
{
    public BorderResistance()
    {
        Left.DisposeWith(this);
        Top.DisposeWith(this);
        Right.DisposeWith(this);
        Bottom.DisposeWith(this);

        this.UnsavedOn(
            e => e.Left,
            e => e.Top,
            e => e.Right,
            e => e.Bottom
        );
    }

    public BorderSide Left { get; } = new();

    public BorderSide Top { get; } = new();

    public BorderSide Right { get; } = new();

    public BorderSide Bottom { get; } = new();

    IBorderSide IBorderResistance.Left => Left;
    IBorderSide IBorderResistance.Top => Top;
    IBorderSide IBorderResistance.Right => Right;
    IBorderSide IBorderResistance.Bottom => Bottom;
}
