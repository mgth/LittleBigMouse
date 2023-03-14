/*
  LittleBigMouse.Daemon
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

  This file is part of LittleBigMouse.Daemon.

    LittleBigMouse.Daemon is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    LittleBigMouse.Daemon is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/

using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Globalization;
using System.Linq;
using System.Runtime.InteropServices;
using System.Windows;
using HLab.Sys.MouseHooker;
using HLab.Sys.Windows.API;
using LittleBigMouse.Zoning;

using Microsoft.Win32;

using static System.Double;
using static HLab.Sys.Windows.API.WinUser;


//using static HLab.Windows.API.NativeMethods;

namespace LittleBigMouse.Daemon;

//[ServiceBehavior(InstanceContextMode = InstanceContextMode.Single)]
public class MouseEngine
{
    readonly ZonesLayout _zones;
    Action _stopAction = null;

    //private readonly IMouseHooker _hook = new MouseHookerWinEvent();
    readonly IMouseHooker _hook = new MouseHookerWindowsHook();


    public MouseEngine(ZonesLayout zones)
    {
        _zones = zones;
    }

    public void Start()
    {
        Stop();

        // TODO check if it's realy better with realtime
        Process.GetCurrentProcess().PriorityClass = ProcessPriorityClass.RealTime;

        if (_zones.AdjustPointer)
        {
            SaveInitialCursor();

            ZoneChanged += AdjustPointer;

            _stopAction += () =>
            {
                ZoneChanged -= AdjustPointer;
                RestoreInitialCursor();
            };
        }

        if (_zones.AdjustSpeed)
        {
            var initialMouseSpeed = GetInitialMouseSpeed();

            ZoneChanged += AdjustSpeed;

            _stopAction += () =>
            {
                ZoneChanged -= AdjustSpeed;
                RestoreMouseSpeed(initialMouseSpeed);
            };
        }

        //if (Layout.HomeCinema)
        //{
        //    ZoneChanged += HomeCinema;
        //    _stopAction += () => ZoneChanged -= HomeCinema;
        //}

        ZoneChanged += VerboseZoneChange;

        _hook.SetMouseMoveAction(OnMouseMoveExtFirst);

        _hook.Hook();

        Console.WriteLine("start");
    }

    const string RootKey = @"\Software\Mgth\LittleBigMouse";

    static void SaveInitialCursor()
    {
        using var key = Registry.CurrentUser.CreateSubKey(RootKey);

        if(key==null) return;

        using var saveKey = key.CreateSubKey("InitialCursor");

        if (saveKey?.ValueCount == 0)
        {
            LbmMouse.SaveCursor(saveKey);
        }
    }

    static void RestoreInitialCursor()
    {
        using var key = Registry.CurrentUser.OpenSubKey(RootKey);

        if(key==null) return;

        using var saveKey = key.OpenSubKey("InitialCursor");

        if (saveKey != null) LbmMouse.RestoreCursor(saveKey);

        key.DeleteSubKey("InitialCursor");

    }

    static double GetInitialMouseSpeed()
    {
        using var key = Registry.CurrentUser.OpenSubKey(RootKey);

        var ms = key?.GetValue("InitialMouseSpeed", string.Empty)?.ToString();

        if (!string.IsNullOrEmpty(ms) && TryParse(ms, out var initialMouseSpeed)) return initialMouseSpeed;

        initialMouseSpeed = LbmMouse.MouseSpeed;
        key?.SetValue("InitialMouseSpeed", initialMouseSpeed.ToString(CultureInfo.InvariantCulture),
            RegistryValueKind.String);

        return initialMouseSpeed;
    }

    static void RestoreMouseSpeed(double mouseSpeed)
    {
        LbmMouse.MouseSpeed = mouseSpeed;
        using var key = Registry.CurrentUser.CreateSubKey(RootKey);

        key?.DeleteValue("InitialMouseSpeed");
    }

    static void VerboseZoneChange(object sender, ZoneChangeEventArgs e)
    {
        Console.WriteLine(e.NewZone.DeviceId);
    }

    public void Stop()
    {
        _stopAction?.Invoke();
        _stopAction = null;

        _hook.UnHook();
        _hook.SetMouseMoveAction(null);

        Process.GetCurrentProcess().PriorityClass = ProcessPriorityClass.Normal;

        Console.WriteLine("stop");
    }


    readonly Stopwatch _timer = new Stopwatch();
    int _count = -10;


    void PrintResult()
    {
        Console.WriteLine("AVG :" + _timer.ElapsedTicks / _count);
        Console.WriteLine("AVG :" + _timer.Elapsed.TotalMilliseconds / _count);
    }


    Point _oldPoint;
    Zone _oldZone;

    void OnMouseMoveExtFirst(object sender, HookMouseEventArg e)
    {
        _oldPoint = e.Point;

        _oldZone = _zones.FromPixel(_oldPoint);

        if (_oldZone == null) return; // TODO : this could indicate a problem with the config

        if (false/*Layout.AllowCornerCrossing*/) // TODO : move options to 'zones'
            _hook.SetMouseMoveAction(MouseMoveCross);
        else
            _hook.SetMouseMoveAction(MouseMoveStraight);
    }

    public event EventHandler<ZoneChangeEventArgs> ZoneChanged;

    [DllImport("user32", SetLastError = true)]
    static extern IntPtr OpenInputDesktop(uint dwFlags,
        bool fInherit,
        uint dwDesiredAccess);
    [DllImport("user32.dll", SetLastError = true)]
    static extern bool SetThreadDesktop(IntPtr hDesktop);
    [DllImport("user32.dll")]
    static extern bool SwitchDesktop(IntPtr hDesktop);
    [DllImport("user32.dll")]
    public static extern IntPtr GetThreadDesktop(int dwThreadId);

    [DllImport("kernel32.dll")]
    public static extern int GetCurrentThreadId();
    enum DESKTOP_ACCESS : uint
    {
        DESKTOP_NONE = 0,
        DESKTOP_READOBJECTS = 0x0001,
        DESKTOP_CREATEWINDOW = 0x0002,
        DESKTOP_CREATEMENU = 0x0004,
        DESKTOP_HOOKCONTROL = 0x0008,
        DESKTOP_JOURNALRECORD = 0x0010,
        DESKTOP_JOURNALPLAYBACK = 0x0020,
        DESKTOP_ENUMERATE = 0x0040,
        DESKTOP_WRITEOBJECTS = 0x0080,
        DESKTOP_SWITCHDESKTOP = 0x0100,

        GENERIC_ALL = (DESKTOP_READOBJECTS | DESKTOP_CREATEWINDOW | DESKTOP_CREATEMENU |
                       DESKTOP_HOOKCONTROL | DESKTOP_JOURNALRECORD | DESKTOP_JOURNALPLAYBACK |
                       DESKTOP_ENUMERATE | DESKTOP_WRITEOBJECTS | DESKTOP_SWITCHDESKTOP),
    }
    [DllImport("user32.dll")]
    static extern bool EnumWindowStations(EnumWindowStationsDelegate lpEnumFunc, IntPtr lParam);

    delegate bool EnumWindowStationsDelegate(string windowsStation, IntPtr lParam);

    static bool EnumWindowStationsCallback(string windowStation, IntPtr lParam)
    {
        GCHandle gch = GCHandle.FromIntPtr(lParam);
        IList<string> list = gch.Target as List<string>;

        if (null == list)
        {
            return (false);
        }

        list.Add(windowStation);

        return (true);
    }


    WinDef.Rect _oldClipRect;
    bool _reset = false;
    Point _goto = new Point(0, 0);

    void MouseMoveStraight(object sender, HookMouseEventArg e)
    {
        var pIn = e.Point;
        if(_reset)
        {
            Debug.WriteLine($"=> : {pIn.X:0},{pIn.Y:0}");
            //LbmMouse.CursorPos = _goto;
            ClipCursor(ref _oldClipRect);
            _reset = false;
        }

        if (_oldZone.ContainsPixel(pIn))
        {
            _oldPoint = pIn;
            e.Handled = false;
            return;
        }

        Point pInMm = _oldZone.PixelsToPhysical(pIn);

        Debug.WriteLine($"=====");
        Debug.WriteLine($"Leaving zone : {_oldZone?.Name??"none"} at {pIn.X:0},{pIn.Y:0} ({pInMm.X:0},{pInMm.Y:0}) mm");

        //Point oldpInMm = _oldZone.Px2Mm(_oldPoint);
        Zone zoneOut = null;

        var minDx = 0.0;
        var minDy = 0.0;

        if (pIn.Y >= _oldZone.PixelsBounds.Bottom)
        {
            Debug.WriteLine("by bottom");
            foreach (var zone in _zones.Zones)
            {
                if (zone.PhysicalBounds.Left > pInMm.X || zone.PhysicalBounds.Right < pInMm.X) continue;

                // Distance to screen
                var dy = zone.PhysicalBounds.Top - _oldZone.PhysicalBounds.Bottom;

                if (dy < 0) continue; // wrong direction

                if (zoneOut != null) // one solution already found
                {
                    if (dy > minDy) continue; // zone farer than already found
                }

                minDy = dy;
                zoneOut = zone;
            }
        }
        else if (pIn.Y < _oldZone.PixelsBounds.Top)
        {
            Debug.WriteLine("by top");
            foreach (var zone in _zones.Zones)
            {
                if (zone.PhysicalBounds.Left > pInMm.X || zone.PhysicalBounds.Right < pInMm.X) continue;

                // Distance to screen
                var dy = zone.PhysicalBounds.Bottom - _oldZone.PhysicalBounds.Top;

                if (dy > 0) continue; // wrong direction
                if (zoneOut != null) // one solution already found
                {
                    if (dy < minDy) continue; // zone farer than already found
                }

                minDy = dy;
                zoneOut = zone;
            }
        }

        if (pIn.X >= _oldZone.PixelsBounds.Right)
        {
            Debug.WriteLine("by right");
            foreach (var zone in _zones.Zones)
            {
                if (zone.PhysicalBounds.Top > pInMm.Y || zone.PhysicalBounds.Bottom < pInMm.Y) continue;

                // Distance to screen
                var dx = zone.PhysicalBounds.Left - _oldZone.PhysicalBounds.Right;

                if (dx < 0) continue; // wrong direction
                if (zoneOut != null) // one solution already found
                {
                    if (dx > minDx) continue; // zone farer than already found
                }

                minDx = dx;
                zoneOut = zone;
            }
        }
        else if (pIn.X < _oldZone.PixelsBounds.Left)
        {
            Debug.WriteLine("by left");
            foreach (var zone in _zones.Zones)
            {
                if (zone.PhysicalBounds.Top > pInMm.Y || zone.PhysicalBounds.Bottom < pInMm.Y) continue;
                var dx = zone.PhysicalBounds.Right - _oldZone.PhysicalBounds.Left;

                if (dx > 0) continue; // wrong direction
                if (zoneOut != null) // one solution already found
                {
                    if (dx < minDx) continue; // zone farer than already found
                }

                minDx = dx;
                zoneOut = zone;
            }
        }

        if (zoneOut == null)
        {
            Debug.WriteLine($"No zone found : {pIn}");

            var r = new WinDef.Rect((int)_oldZone.PixelsBounds.Left, (int)_oldZone.PixelsBounds.Top, (int)_oldZone.PixelsBounds.Right,
                (int)_oldZone.PixelsBounds.Bottom);

            GetClipCursor(out _oldClipRect);
            _reset = true;
            //_goto = pIn;

            ClipCursor(ref r);

            e.Handled = false; // when set to true, cursor stick to frame
            return;
        }
        else
        {
            Debug.WriteLine($"=====");

            var pMm = new Point(pInMm.X + minDx, pInMm.Y + minDy);
            var pOut = zoneOut.PhysicalToPixels(pMm);
            pOut = zoneOut.InsidePixelsBounds(pOut);

            Debug.WriteLine($"to : {zoneOut.Name} at {pOut.X:0},{pOut.Y:0} ({pMm.X:0},{pMm.Y:0}) mm");

            var travel = _oldZone.TravelPixels(_zones.Zones,zoneOut);

            _oldZone = zoneOut.Main;
            _oldPoint = pOut;

            //LbmMouse.CursorPos = new Point(1.0,1.0);


            var r = new WinDef.Rect(
                (int) zoneOut.PixelsBounds.Left ,//+1, 
                (int) zoneOut.PixelsBounds.Top ,//+1, 
                (int) (zoneOut.PixelsBounds.Right + 1/*-1+1*/),
                (int) (zoneOut.PixelsBounds.Bottom + 1/*-1+1*/)
                );

            //NativeMethods.ClipCursor(ref r0);
            //LbmMouse.CursorPos = pOut;


            //NativeMethods.ClipCursor(ref r);
            //_reset = true;
            //_goto = pOut;
            GetClipCursor(out _oldClipRect);

            var pos = pIn;

            foreach (var z in travel)
            {
                Debug.WriteLine($"travel : {z}");
                if(z.Contains(pos)) continue;
                var rect = new WinDef.Rect(z);
                ClipCursor(ref rect);
                pos = LbmMouse.CursorPos;
                //LbmMouse.CursorPos = pos = new Point((z.Right + z.Left)/2, (z.Top + z.Bottom)/2);
                if(z.Contains(pOut)) break;
            }


            //LbmMouse.CursorPos = zoneOut.CenterPixel;
            ClipCursor(ref r);
            _reset = true;
            LbmMouse.CursorPos = pOut;
            ClipCursor(ref _oldClipRect);

            var p = LbmMouse.CursorPos;


            if (false && (Math.Abs(pOut.X - p.X) >= 1.0 || Math.Abs(pOut.Y - p.Y) >= 1.0))
            {
                var dist = (pOut - p).Length;

                Debug.WriteLine($"failed 1 : {dist}");
            }

            //{            
            //    Debug.WriteLine($"failed 1 : {p}");

            //    LbmMouse.CursorPos = pOut;
            //    p = LbmMouse.CursorPos;

            //    if ((Math.Abs(pOut.X - p.X) >= 1.0 || Math.Abs(pOut.Y - p.Y) >= 1.0))
            //    {            
            //        Debug.WriteLine($"failed 2 : {p}");
            //        foreach(var z in _zones.All)
            //        {   
                        
            //            LbmMouse.CursorPos = z.CenterPx;
            //            LbmMouse.CursorPos = pOut;

            //            p = LbmMouse.CursorPos;
            //            if ((Math.Abs(pOut.X - p.X) >= 1.0 || Math.Abs(pOut.Y - p.Y) >= 1.0))
            //            {            
            //               Debug.WriteLine($"failed {z.Name} : {p}");

            //            }
            //            else 
            //            {
            //                Debug.WriteLine($"ok {z.Name} : {p}");
            //                break;
            //            }
            //        }
            //    }
            //    else Debug.WriteLine($"ok : {p}");


            //}
            //else Debug.WriteLine($"ok : {p}");

            e.Handled = true;
            ZoneChanged?.Invoke(this, new ZoneChangeEventArgs(_oldZone, zoneOut));
            return;
        }

        //IntPtr hwnd = OpenInputDesktop(0, false, DESKTOP_SWITCHDESKTOP);
        //SetThreadDesktop(hwnd);

        //var movement = new INPUT { Type = (UInt32)0 };
        //movement.Data.Mouse.Flags = (UInt32)(MouseFlag.Move | MouseFlag.Absolute | MouseFlag.VirtualDesk);
        //movement.Data.Mouse.X = (int)pOut.X;
        //movement.Data.Mouse.Y = (int)pOut.Y;

        //INPUT[] inputs = {movement};

        //SendInput((UInt32)inputs.Length, inputs, Marshal.SizeOf(typeof(INPUT)));




        //var p = LbmMouse.CursorPos;
        //while (Math.Abs(pOut.X - p.X) >= 1 || Math.Abs(pOut.Y - p.Y) >= 1)
        //{
        //    LbmMouse.CursorPos = pOut;
        //    //e.Point = pOut;
        //    p = LbmMouse.CursorPos;
        //}

        //if (Math.Abs(pOut.X - p.X) >= 1 || Math.Abs(pOut.Y - p.Y) >= 1)
        //{
        //    Debug.WriteLine("Mouse move did not work");

        //    IntPtr hOldDesktop = GetThreadDesktop(GetCurrentThreadId());
        //    IntPtr hwnd = OpenInputDesktop(0, true, (uint)DESKTOP_ACCESS.GENERIC_ALL);

        //    Thread t = new Thread(() =>
        //    {
        //        SwitchDesktop(hwnd);
        //        var b = SetThreadDesktop(hwnd);

        //        var b2 = LbmMouse.MouseEvent(
        //            NativeMethods.MOUSEEVENTF.ABSOLUTE | NativeMethods.MOUSEEVENTF.MOVE
        //            | NativeMethods.MOUSEEVENTF.VIRTUALDESK
        //            , pOut.X, pOut.Y);
        //        if (b2 == 0)
        //        {
        //            var s = NativeMethods.GetLastError();
        //        }

        //        //LbmMouse.CursorPos = pOut;
        //        var b3 = NativeMethods.SetCursorPos((int)pOut.X, (int)pOut.Y);

        //        if (b3 == false)
        //        {
        //            var s = NativeMethods.GetLastError();
        //        }

        //        //    IList<string> list = new List<string>();
        //        //    GCHandle gch = GCHandle.Alloc(list);
        //        //    EnumWindowStationsDelegate childProc = new EnumWindowStationsDelegate(EnumWindowStationsCallback);

        //        //    EnumWindowStations(childProc, GCHandle.ToIntPtr(gch));


        //        //}
        //    });

        //    t.Start();
        //    t.Join();

        //    SwitchDesktop(hOldDesktop);

        //    //var w = new Window
        //    //{
        //    //    WindowStyle = WindowStyle.None,
        //    //    Visibility = Visibility.Collapsed,
        //    //    Width = 0,
        //    //    Height = 0
        //    //};
        //    //w.Show();

        //    ////const int DESKTOP_SWITCHDESKTOP = 256;
        //    ////IntPtr hwnd = OpenInputDesktop(0, false, 0x00020000);
        //    ////var b = SetThreadDesktop(hwnd);

        //    //LbmMouse.CursorPos = pOut;

        //    //w.Close();
        //}

        e.Handled = true;
    }

    void MouseMoveCross(object sender, HookMouseEventArg e)
    {
        // TODO : if (e.Clicked) return;
        var pIn = e.Point;

        if (_oldZone.ContainsPixel(pIn))
        {
            _oldPoint = pIn;
            e.Handled = false;
            return;
        }

        var pInMmOld = _oldZone.PixelsToPhysical(_oldPoint);
        var pInMm = _oldZone.PixelsToPhysical(pIn);
        Zone zoneOut = null;

        var trip = new Segment(pInMmOld, pInMm);
        var minDist = PositiveInfinity;

        var pOutInMm = pInMm;

        foreach (var zone in _zones.Zones.Where(z => !ReferenceEquals(z, _oldZone)))
        {
            foreach (var p in trip.Line.Intersect(zone.PhysicalBounds))
            {
                var travel = new Segment(pInMmOld, p);
                if (!travel.Rect.Contains(pInMm)) continue;
                var dist = travel.SizeSquared;
                if (dist > minDist) continue;

                minDist = dist;
                zoneOut = zone;
                pOutInMm = p;
            }
        }

        if (zoneOut == null)
        {
            LbmMouse.CursorPos = _oldZone.InsidePixelsBounds(pIn);
            e.Handled = true;
            return;
        }

        var pOut = zoneOut.PhysicalToPixels(pOutInMm);
        pOut = zoneOut.InsidePixelsBounds(pOut);
        _oldZone = zoneOut.Main;
        _oldPoint = pOut;
        LbmMouse.CursorPos = pOut;
        ZoneChanged?.Invoke(this, new ZoneChangeEventArgs(_oldZone, zoneOut));
        e.Handled = true;
        return;

        //}

        //finally
        //{
        //    if (_count >= 0) _timer.Stop();
        //    _count++;
        //}
    }

    public class ZoneChangeEventArgs : EventArgs
    {
        public ZoneChangeEventArgs(Zone oldZone, Zone newZone)
        {
            OldZone = oldZone;
            NewZone = newZone;
        }

        public Zone OldZone { get; }
        public Zone NewZone { get; }
    }

    void AdjustPointer(object sender, ZoneChangeEventArgs args)
    {
        if (args.NewZone.Dpi - args.OldZone.Dpi < 1) return;
        if (args.NewZone.Dpi > 110)
        {
            LbmMouse.SetCursorAero(args.NewZone.Dpi > 138 ? 3 : 2);
        }
        else LbmMouse.SetCursorAero(1);
    }

    void AdjustSpeed(object sender, ZoneChangeEventArgs args)
    {
        LbmMouse.MouseSpeed = Math.Round((5.0 / 96.0) * args.NewZone.Dpi, 0);
    }

    void HomeCinema(object sender, ZoneChangeEventArgs args)
    {
        // TODO
        //args.OldZone.Screen.Monitor.Vcp().Power = false;
        //args.NewZone.Screen.Monitor.Vcp().Power = true;
    }


    public void MatchConfig(string configId)
    {
        //Layout.MatchLayout(configId);
    }

    public bool Running => _hook.Hooked;


}

