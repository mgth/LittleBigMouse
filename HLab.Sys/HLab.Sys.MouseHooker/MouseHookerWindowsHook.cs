using System;
using System.Collections.Concurrent;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Threading;
using System.Windows;

namespace HLab.Sys.MouseHooker
{
    public class MouseHookerWindowsHook : MouseHooker
    {
        private static IntPtr _hookId = IntPtr.Zero;

        public override void UnHook()
        {
            if (!Hooked()) return;
            if(UnhookWindowsHookEx(_hookId))
                _hookId = IntPtr.Zero;
        }

        public override bool Hooked() => !(_hookId == IntPtr.Zero);


        private const int WH_MOUSE_LL = 14;

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


        private LowLevelMouseProc _hookCallback;
        public override void Hook()
        {
            if (Hooked()) return;

            _hookCallback = new LowLevelMouseProc(HookCallback);

            //_hookId = SetWindowsHookEx(WH_MOUSE_LL, HookCallback,IntPtr.Zero, 0);
            using var curProcess = Process.GetCurrentProcess();
            using var curModule = curProcess.MainModule;
            if (curModule != null)
            {
                _hookId = SetWindowsHookEx(WH_MOUSE_LL, _hookCallback,
                    /*GetModuleHandle(curModule.ModuleName),*/IntPtr.Zero, 0);
            }
        }

        private POINT _oldLocation;
        private readonly ConcurrentQueue<HookMouseEventArg> _queue = new ConcurrentQueue<HookMouseEventArg>();


        private readonly object _lock = new object();
        private IntPtr HookCallback(int nCode, MouseMessages wParam, IntPtr lParam)
        {
            lock (_lock)
            {
                if (nCode >= 0 && lParam!=IntPtr.Zero)
                {
                    if ( (wParam & MouseMessages.WM_MOUSEMOVE) != 0)
                    {
                        int x;
                        int y;
                        unsafe
                        {
                            x = ((MSLLHOOKSTRUCT*)lParam)-> /*pt.*/x;
                            y = ((MSLLHOOKSTRUCT*)lParam)-> /*pt.*/y;
                        }

                        if (_oldLocation.x != x || _oldLocation.y != y)
                        {
                            _oldLocation = new POINT {x=x,y=y};

                            var p = new HookMouseEventArg
                            {
                                Point = new Point(x, y),
                                Handled = false
                            };
                            OnMouseMove(p);
                            if (p.Handled) return new IntPtr(-1);
                        }
                        

                        //    var prm = Marshal.PtrToStructure<MSLLHOOKSTRUCT>(lParam);
                        //if (
                        //    _oldLocation.x != hookStruct.pt.x
                        //    || _oldLocation.y != hookStruct.pt.y
                        //    )
                        //{
                        //    _oldLocation = hookStruct.pt;

                        //    var p = new HookMouseEventArg
                        //    {
                        //        Point = new Point(hookStruct.pt.x, hookStruct.pt.y),
                        //        Handled = false
                        //    };
                        //    if (p.Handled) return new IntPtr(-1);
                        //}
                    }
                    else
                    {
                        
                    }
                }

            }

            var r = CallNextHookEx(_hookId, nCode, wParam, lParam);

            return r;
        }
    }
}