using Avalonia.Input;
using HLab.Base.Avalonia.Controls;
using LittleBigMouse.Plugin.Layout.Avalonia;
using Xunit;

namespace LittleBigMouse.Ui.Avalonia.Tests;

public sealed class WheelPointerCaptureTests
{
    [Fact]
    public void KeepsWheelPointerCapturedByTheEditor()
    {
        using var pointer = new Pointer(1, PointerType.Mouse, true);
        var editor = new DoubleBox();
        var capture = new WheelPointerCapture();

        capture.KeepOn(editor, pointer);

        Assert.Same(editor, pointer.Captured);
    }

    [Fact]
    public void ReleaseReturnsPointerToNormalHitTesting()
    {
        using var pointer = new Pointer(1, PointerType.Mouse, true);
        var editor = new DoubleBox();
        var capture = new WheelPointerCapture();
        capture.KeepOn(editor, pointer);

        capture.Release();

        Assert.Null(pointer.Captured);
    }

    [Fact]
    public void ANewWheelTargetReplacesThePreviousCapture()
    {
        using var pointer = new Pointer(1, PointerType.Mouse, true);
        var first = new DoubleBox();
        var second = new DoubleBox();
        var capture = new WheelPointerCapture();
        capture.KeepOn(first, pointer);

        capture.KeepOn(second, pointer);

        Assert.Same(second, pointer.Captured);
    }
}
