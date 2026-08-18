using HLab.Base.Avalonia.Controls;
using LittleBigMouse.Plugin.Layout.Avalonia.SizePlugin;
using Xunit;

namespace LittleBigMouse.Ui.Avalonia.Tests;

public sealed class DimensionTextBoxTests
{
    sealed class InspectableBorderEditor : NonNegativeDoubleBox
    {
        public Type EffectiveStyleKey => StyleKeyOverride;
    }

    [Fact]
    public void BorderEditorCoercesNegativeValuesToZero()
    {
        var editor = new NonNegativeDoubleBox { Value = 12.5 };

        editor.Value = -1;

        Assert.Equal(0, editor.Value);
    }

    [Fact]
    public void BorderEditorPreservesPositiveValues()
    {
        var editor = new NonNegativeDoubleBox { Value = 12.5 };

        Assert.Equal(12.5, editor.Value);
    }

    [Fact]
    public void BorderEditorUsesTheStandardDoubleBoxStyle()
    {
        var editor = new InspectableBorderEditor();

        Assert.Equal(typeof(DoubleBox), editor.EffectiveStyleKey);
    }
}
