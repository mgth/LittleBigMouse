using Avalonia;
using HLab.Base.Avalonia.Controls;

namespace LittleBigMouse.Plugin.Layout.Avalonia.SizePlugin;

/// <summary>
/// Numeric editor for dimensions that cannot be negative. Coercing the control value
/// keeps rejected input from remaining visible when the model is already at zero and
/// therefore has no property change to emit back to the binding.
/// </summary>
internal class NonNegativeDoubleBox : DoubleBox
{
    // DoubleBox's theme uses an exact type selector. Reuse its style key so this
    // specialized editor keeps the standard template instead of rendering empty.
    protected override Type StyleKeyOverride => typeof(DoubleBox);

    protected override void OnPropertyChanged(AvaloniaPropertyChangedEventArgs change)
    {
        base.OnPropertyChanged(change);

        if (change.Property == ValueProperty && Value < 0)
            SetCurrentValue(ValueProperty, 0.0);
    }
}
