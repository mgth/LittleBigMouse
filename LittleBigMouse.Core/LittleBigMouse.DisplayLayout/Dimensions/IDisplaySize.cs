
using System;
using HLab.Base;
using HLab.Geo;

namespace LittleBigMouse.DisplayLayout.Dimensions;

/// <summary>
/// Read-only view of display dimensions. Computed dimensions expose this contract so
/// consumers cannot request a mutation that the implementation cannot perform.
/// </summary>
public interface IDisplaySize : IEquatable<IDisplaySize>, ISavable
{
    IDisposable DelayChangeNotifications();

    double Width { get; }
    double Height { get; }
    double X { get; }
    double Y { get; }
    double TopBorder { get; }
    double BottomBorder { get; }
    double LeftBorder { get; }
    double RightBorder { get; }

    Rect Bounds { get; }
    Point Center { get; }

    Rect OutsideBounds { get; }
    double OutsideWidth { get; }
    double OutsideHeight { get; }
    double OutsideX { get; }
    double OutsideY { get; }

    Point Location { get; }
}

/// <summary>
/// Display dimensions whose content bounds can be explicitly edited. Fixed borders,
/// such as the zero pixel border, remain read-only through this contract.
/// </summary>
public interface IMutableDisplayBounds : IDisplaySize
{
    new double Width { get; set; }
    new double Height { get; set; }
    new double X { get; set; }
    new double Y { get; set; }
}

/// <summary>
/// Fully mutable display dimensions, including physical borders.
/// </summary>
public interface IMutableDisplaySize : IMutableDisplayBounds
{
    new double TopBorder { get; set; }
    new double BottomBorder { get; set; }
    new double LeftBorder { get; set; }
    new double RightBorder { get; set; }
}
