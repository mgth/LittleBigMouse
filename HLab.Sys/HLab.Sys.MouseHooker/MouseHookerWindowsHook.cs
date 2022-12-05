using System;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Windows;

namespace HLab.Sys.MouseHooker;

public class MouseHookerWindowsHook : MouseHooker
{

    //############################################################################

    #region IMouseHooker

    public override bool Hook()
    {
        if (Hooked) return true;

        //_hookCallback =  new LowLevelMouseProc(HookCallback);;

        //_hookId = SetWindowsHookEx(WH_MOUSE_LL, _hookCallback, IntPtr.Zero, 0);

        //return Hooked;

        _hookCallback = new LowLevelMouseProc(HookCallback);

        _hookHandle = GCHandle.Alloc(_hookCallback);

        //_hookId = SetWindowsHookEx(WH_MOUSE_LL, HookCallback,IntPtr.Zero, 0);
        using var curProcess = Process.GetCurrentProcess();
        using var curModule = curProcess.MainModule;
        if (curModule != null)
        {
            _hookId = SetWindowsHookEx(WH_MOUSE_LL, _hookCallback,
                GetModuleHandle(curModule.ModuleName),/*IntPtr.Zero,*/ 0);
        }

        return Hooked;
    }

    public override bool UnHook()
    {
        if (!Hooked) return true;
        if (UnhookWindowsHookEx(_hookId))
        {
            _hookHandle.Free();
            _hookId = IntPtr.Zero;
        }
        
        return !Hooked;
    }

    public override bool Hooked => _hookId != IntPtr.Zero;

    //[MethodImpl(MethodImplOptions.AggressiveInlining)]
    //public void SetMouseMoveAction(Action<IMouseHooker, HookMouseEventArg> action)
    //{
    //    MouseMoveAction = action;
    //}

    #endregion IMouseHooker

    //############################################################################

    #region Private variables

    private static IntPtr _hookId = IntPtr.Zero;
    private LowLevelMouseProc _hookCallback;
    private GCHandle _hookHandle;

    private static int _oldX, _oldY;

    private static readonly object _lock = new();
    private static readonly IntPtr NoPtr = new(-1);

    #endregion Private variables

    //############################################################################

    #region Main Code

    private static IntPtr HookCallback(int nCode, MouseMessages wParam, IntPtr lParam)
    {
        Console.WriteLine($"Callback {nCode}");
        lock (_lock)
        {
            if (nCode < 0) goto CallNext;
            if (lParam == IntPtr.Zero) goto CallNext;
            if ((wParam & MouseMessages.WM_MOUSEMOVE) == 0) goto CallNext;

            int x;
            int y;
            unsafe
            {
                x = ((MSLLHOOKSTRUCT*)lParam)->x;
                y = ((MSLLHOOKSTRUCT*)lParam)->y;
            }

            if (_oldX == x && _oldY == y) goto CallNext;

            _oldX = x;
            _oldY = y;

            var p = new HookMouseEventArg
            {
                Point = new Point(x, y),
                Handled = false
            };

            OnMouseMove(p);

            if (p.Handled) return NoPtr;

            CallNext:
            return CallNextHookEx(_hookId, nCode, wParam, lParam);
        }
    }

    //protected void OnMouseMove(HookMouseEventArg args)
    //{
    //    Console.WriteLine($"Move {args.Point}");
    //    //MouseMove?.Invoke(this,args);
    //    MouseMoveAction?.Invoke(this, args);
    //}

    //protected Action<IMouseHooker, HookMouseEventArg> MouseMoveAction;

    #endregion Main code

    //############################################################################

    #region Native Functions

    private const int WH_MOUSE_LL = 14;
    private const int WH_MOUSE = 7;

    [Flags]
    private enum MouseMessages
    {
        WM_LBUTTONDOWN = 0x0201,
        WM_LBUTTONUP = 0x0202,
        WM_MOUSEMOVE = 0x0200,
        WM_MOUSEWHEEL = 0x020A,
        WM_RBUTTONDOWN = 0x0204,
        WM_RBUTTONUP = 0x0205
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct POINT
    {
        public int x;
        public int y;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct MSLLHOOKSTRUCT
    {
        //            public POINT pt;
        public int x;
        public int y;
        public uint mouseData;
        public uint flags;
        public uint time;
        public IntPtr dwExtraInfo;
    }

    [DllImport("user32.dll", CharSet = CharSet.Auto, SetLastError = true)]
    private static extern IntPtr SetWindowsHookEx(int idHook, LowLevelMouseProc lpfn, IntPtr hMod, uint dwThreadId);


    [DllImport("user32.dll", CharSet = CharSet.Auto, SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool UnhookWindowsHookEx(IntPtr hhk);


    [DllImport("user32.dll", CharSet = CharSet.Auto, SetLastError = true)]
    private static extern IntPtr CallNextHookEx(IntPtr hhk, int nCode, MouseMessages wParam, IntPtr lParam);


    [DllImport("kernel32.dll", CharSet = CharSet.Auto, SetLastError = true)]
    private static extern IntPtr GetModuleHandle(string lpModuleName);


    private delegate IntPtr LowLevelMouseProc(int nCode, MouseMessages wParam, IntPtr lParam);
    private delegate IntPtr MouseProc(int nCode, MouseMessages wParam, IntPtr lParam);


    #endregion




    //private readonly ConcurrentQueue<HookMouseEventArg> _queue = new();




}
