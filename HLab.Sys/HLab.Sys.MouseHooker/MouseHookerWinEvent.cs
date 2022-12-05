using System;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Windows;

namespace HLab.Sys.MouseHooker;

public class MouseHookerWinEvent : IMouseHooker
{

    //############################################################################

    #region IMouseHooker
    public bool Hook()
    {
        UnHook();
        _hookId = SetWinEventHook(EVENT_OBJECT_LOCATIONCHANGE, EVENT_OBJECT_LOCATIONCHANGE,
                IntPtr.Zero, HookCallback, (uint)0/*curProcess.Id*/, (uint)0, WINEVENT_OUTOFCONTEXT);
        return Hooked;
    }

    public bool UnHook()
    {
        if (_hookId == IntPtr.Zero) return true;

        if(UnhookWinEvent(_hookId)) 
            _hookId = IntPtr.Zero;

        return !Hooked;
    }
    
    public bool Hooked => _hookId != IntPtr.Zero;

    [MethodImpl(MethodImplOptions.AggressiveInlining)]
    public void SetMouseMoveAction(Action<IMouseHooker, HookMouseEventArg> action)
    {
        MouseMoveAction = action;
    }

    #endregion IMouseHooker

    //############################################################################

    #region private variables

    private static IntPtr _hookId = IntPtr.Zero;
    private POINT _oldLocation;

    #endregion private variables

    //############################################################################

    #region Main code

    private void HookCallback(IntPtr hWinEventHook, uint eventType, IntPtr hwnd, int idObject, int idChild, uint dwEventThread, uint dwmsEventTime)
    {
        if (hwnd != IntPtr.Zero) return;

        if (!GetCursorPos(out var p)) return;

        if (_oldLocation.X == p.X && _oldLocation.Y == p.Y) return;

        _oldLocation = p;

        OnMouseMove(new HookMouseEventArg { Point = p });
    }

    protected void OnMouseMove(HookMouseEventArg args)
    {
        //MouseMove?.Invoke(this,args);
        MouseMoveAction?.Invoke(this, args);
    }

    protected Action<IMouseHooker, HookMouseEventArg> MouseMoveAction;

    #endregion Main code

    //############################################################################

    #region Native Functions

    public delegate void WinEventDelegate(IntPtr hWinEventHook, uint eventType, IntPtr hwnd, int idObject, int idChild, uint dwEventThread, uint dwmsEventTime);
    private const uint WINEVENT_OUTOFCONTEXT = 0x0000;
    private const uint WINEVENT_INCONTEXT = 0x0004;
    private const uint EVENT_OBJECT_LOCATIONCHANGE = 0x800B;

    [DllImport("user32.dll")]
    public static extern IntPtr SetWinEventHook(uint eventMin, uint eventMax, IntPtr hmodWinEventProc, WinEventDelegate lpfnWinEventProc, uint idProcess, uint idThread, uint dwFlags);

    [DllImport("user32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool UnhookWinEvent(IntPtr hWinEventHook);

    [StructLayout(LayoutKind.Sequential)]
    public struct POINT
    {
        public int X;
        public int Y;

        public static implicit operator Point(POINT point)
        {
            return new Point(point.X, point.Y);
        }
    }
    [DllImport("user32.dll")]
    public static extern bool GetCursorPos(out POINT lpPoint);

    #endregion
}

