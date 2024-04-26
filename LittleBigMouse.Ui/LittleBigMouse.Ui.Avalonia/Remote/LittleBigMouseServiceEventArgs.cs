using System;
using LittleBigMouse.Zoning;

namespace LittleBigMouse.Ui.Avalonia.Remote;

public class LittleBigMouseServiceEventArgs(LittleBigMouseEvent evt, string payload) : EventArgs
{
    public LittleBigMouseEvent Event { get; } = evt;
    public string Payload { get; } = payload;
}