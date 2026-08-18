using Avalonia.Input;

namespace LittleBigMouse.Plugin.Layout.Avalonia;

/// <summary>
/// Keeps a wheel interaction routed to the same editor when changing the model
/// moves that editor away from the stationary pointer. The capture ends as soon
/// as the user moves or presses the pointer.
/// </summary>
internal sealed class WheelPointerCapture
{
    IPointer? _pointer;
    InputElement? _target;

    public void KeepOn(InputElement target, IPointer pointer)
    {
        if (ReferenceEquals(_target, target) && ReferenceEquals(_pointer, pointer))
        {
            if (!ReferenceEquals(pointer.Captured, target)) pointer.Capture(target);
            return;
        }

        Release();

        _pointer = pointer;
        _target = target;

        target.PointerMoved += OnPointerMoved;
        target.PointerPressed += OnPointerPressed;
        target.PointerCaptureLost += OnPointerCaptureLost;

        pointer.Capture(target);
    }

    public void Release()
    {
        var pointer = _pointer;
        var target = _target;

        _pointer = null;
        _target = null;

        if (target != null)
        {
            target.PointerMoved -= OnPointerMoved;
            target.PointerPressed -= OnPointerPressed;
            target.PointerCaptureLost -= OnPointerCaptureLost;
        }

        if (pointer != null && ReferenceEquals(pointer.Captured, target))
            pointer.Capture(null);
    }

    void OnPointerMoved(object? sender, PointerEventArgs e)
    {
        if (ReferenceEquals(e.Pointer, _pointer)) Release();
    }

    void OnPointerPressed(object? sender, PointerPressedEventArgs e)
    {
        if (ReferenceEquals(e.Pointer, _pointer)) Release();
    }

    void OnPointerCaptureLost(object? sender, PointerCaptureLostEventArgs e)
    {
        var target = _target;
        _pointer = null;
        _target = null;

        if (target == null) return;
        target.PointerMoved -= OnPointerMoved;
        target.PointerPressed -= OnPointerPressed;
        target.PointerCaptureLost -= OnPointerCaptureLost;
    }
}
