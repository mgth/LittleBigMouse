using System;
using HLab.Base.ReactiveUI;
using LittleBigMouse.Zoning;
using ReactiveUI;

namespace LittleBigMouse.DisplayLayout.Dimensions;

/// <summary>
/// A stretch of one monitor edge with its own resistances. See <see cref="IBorderSection"/>
/// for the coordinate convention.
/// <para>
/// Resistances cannot go negative. The daemon already floors them at zero — a
/// negative resistance is not a repulsion, it is simply no resistance — so allowing
/// one here would only create a disagreement: the editor showing -5 mm as though it
/// governed something while the hook does nothing with it.
/// </para>
/// </summary>
public class BorderSection : SavableReactiveModel, IBorderSection
{
    public double From { get; set => SetPositive(ref field, value); }

    public double To { get; set => SetPositive(ref field, value); }

    public double Move { get; set => SetPositive(ref field, value); }

    public bool MoveBlock { get; set => SetUnsavedValue(ref field, value); }

    public double Drag { get; set => SetPositive(ref field, value); }

    public bool DragBlock { get; set => SetUnsavedValue(ref field, value); }

    /// <summary>
    /// Store the value floored at zero, and notify even when the floor swallowed the
    /// change: otherwise a bound editor keeps displaying the number it refused, which
    /// is the very mismatch this is here to prevent.
    /// </summary>
    void SetPositive(ref double backingField, double value, [System.Runtime.CompilerServices.CallerMemberName] string? propertyName = null)
    {
        var floored = Math.Max(0, value);

        if (!SetUnsavedValue(ref backingField, floored, propertyName!)
            && !floored.Equals(value))
        {
            this.RaisePropertyChanged(propertyName);
        }
    }
}
