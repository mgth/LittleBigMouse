/*
  LittleBigMouse.Daemon
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

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
using System.ServiceModel;
using System.ServiceModel.Description;
using System.Threading;
using System.Windows;
using HLab.Windows.API;
using HLab.Windows.MonitorVcp;
using HLab.Windows.Monitors;
using LittleBigMouse.ScreenConfigs;
using Microsoft.Win32;
using MouseKeyboardActivityMonitor;
using MouseKeyboardActivityMonitor.WinApi;

namespace LittleBigMouse_Daemon
{
    [ServiceBehavior(InstanceContextMode = InstanceContextMode.Single)]
    public class MouseEngine
    {
        public ScreenConfig Config { get; private set; }

        private Zones _zones;

        private double _initMouseSpeed;
        public readonly MouseHookListener Hook = new MouseHookListener(new GlobalHooker());

        private ServiceHost _host;

        public void StartServer(ILittleBigMouseService service)
        {
            if (_host == null)
            {
                _host = new ServiceHost(service, LittleBigMouseClient.Address);
                ServiceMetadataBehavior smb = new ServiceMetadataBehavior
                {
                    MetadataExporter = { PolicyVersion = PolicyVersion.Policy15 }
                };

                _host.Description.Behaviors.Add(smb);
            }

            _host.Open();
        }

        public void StopServer()
        {
            _host.Close();
        }

        public void Quit()
        {
            throw new NotImplementedException("MouseEngine : Quit");
        }

        public void Start()
        {
            Stop();

            LoadConfig();

            if (Config == null || !Config.Enabled) return;

            using (RegistryKey key = ScreenConfig.OpenRootRegKey(true))
            {
                string ms = key.GetValue("InitialMouseSpeed", string.Empty).ToString();

                if (string.IsNullOrEmpty(ms))
                {
                    _initMouseSpeed = LbmMouse.MouseSpeed;
                    key.SetValue("InitialMouseSpeed", _initMouseSpeed.ToString(CultureInfo.InvariantCulture),
                        RegistryValueKind.String);
                }
                else
                    double.TryParse(ms, out _initMouseSpeed);

                using (RegistryKey savekey = key.CreateSubKey("InitialCursor"))
                {
                    if (savekey?.ValueCount == 0)
                    {
                        LbmMouse.SaveCursor(savekey);
                    }
                }
            }

            Process.GetCurrentProcess().PriorityClass = ProcessPriorityClass.RealTime;


            _handler = OnMouseMoveExtFirst;
            Hook.MouseMoveExt += _handler;

            if (Config.AdjustPointer)
                ZoneChanged += AdjustPointer;

            if (Config.AdjustSpeed)
                ZoneChanged += AdjustSpeed;

            if(Config.HomeCinema)
                ZoneChanged += HomeCinema;


            Hook.Enabled = true;
        }

        public void Stop()
        {
            if (!Hook.Enabled) return;

            Hook.MouseMoveExt -= _handler;

            if (Config.AdjustPointer)
                ZoneChanged -= AdjustPointer;

            if (Config.AdjustSpeed)
                ZoneChanged -= AdjustSpeed;

            if (Config.HomeCinema)
                ZoneChanged -= HomeCinema;


            Hook.Enabled = false;

            if (Config == null) return;

            if (Config.AdjustSpeed)
            {
                LbmMouse.MouseSpeed = _initMouseSpeed;
                using (var key = ScreenConfig.OpenRootRegKey(true))
                {
                    key.DeleteValue("InitialMouseSpeed");
                }
            }

            if (Config.AdjustPointer)
            {
                using (var key = ScreenConfig.OpenRootRegKey())
                {
                    using (RegistryKey savekey = key.OpenSubKey("InitialCursor"))
                    {
                        if (savekey != null)
                        {
                            LbmMouse.RestoreCursor(savekey);
                        }
                    }
                    key.DeleteSubKey("InitialCursor");
                }
            }

            Process.GetCurrentProcess().PriorityClass = ProcessPriorityClass.Normal;
        }


        public void LoadConfig(ScreenConfig config)
        {
            Config = config;
            ConfigLoaded?.Invoke(config, null);
        }

        public event EventHandler ConfigLoaded;

        public void LoadConfig()
        {
            LoadConfig(new ScreenConfig(MonitorsService.D));

            _zones = new Zones();
            foreach (var screen in Config.AllScreens)
            {
                _zones.Add(new Zone(screen));
            }


            if (Config.LoopX)
            {
                foreach (var screen in Config.AllScreens)
                {
                    var main = _zones.Main.FirstOrDefault(e => ReferenceEquals(e.Screen, screen));
                    _zones.Add(new Zone(screen,main,-Config.PhysicalOutsideBounds.Width,0));
                    _zones.Add(new Zone(screen,main,Config.PhysicalOutsideBounds.Width,0));
                }
            }

            if (Config.LoopY)
            {
                foreach (var screen in Config.AllScreens)
                {
                    var main = _zones.Main.FirstOrDefault(e => ReferenceEquals(e.Screen,screen));
                    _zones.Add(new Zone(screen,main, 0,-Config.PhysicalOutsideBounds.Height));
                    _zones.Add(new Zone(screen,main, 0,Config.PhysicalOutsideBounds.Height));
                }
            }
        }

        private readonly Stopwatch _timer = new Stopwatch();
        private int _count = -10;

        //private void OnMouseMoveExt(object sender, MouseEventExtArgs e)
        //{
        //    //_timer.Start();
        //    //try
        //    //{
        //    if (e.Clicked) return;
        //    var pIn = new Point(e.X, e.Y);

        //    if (_oldZone.ContainsPx(pIn))
        //    {
        //        _oldPoint = pIn;
        //        e.Handled = false;
        //        return;
        //    }

        //    e.Handled = _handler(pIn);

        //    //}
        //    //finally
        //    //{
        //    //    _timer.Stop();
        //    //    _count++;               
        //    //}
        //}

        private void PrintResult()
        {
            Console.WriteLine("AVG :" + _timer.ElapsedTicks / _count);
            Console.WriteLine("AVG :" + _timer.Elapsed.TotalMilliseconds / _count);
        }


        private EventHandler<MouseEventExtArgs> _handler;
        private Point _oldPoint;
        private Zone _oldZone;

        private void OnMouseMoveExtFirst(object sender, MouseEventExtArgs e)
        {
            _oldPoint = new Point(e.X,e.Y);
            //_oldScreenRect = Config.ScreenFromPixel(_oldPoint).InPixel.Bounds;
            _oldZone = _zones.FromPx(_oldPoint);

            Hook.MouseMoveExt -= _handler;

            if (Config.AllowCornerCrossing)
                _handler = MouseMoveCross;
            else
            {
                _handler = MouseMovestraight;
            }

            Hook.MouseMoveExt += _handler;

            e.Handled = false;
        }

        public event EventHandler<ZoneChangeEventArgs> ZoneChanged;

        private void MouseMovestraight(object sender, MouseEventExtArgs e)
        {
            if (e.Clicked) return;
            var pIn = new Point(e.X, e.Y);

            if (_oldZone.ContainsPx(pIn))
            {
                _oldPoint = pIn;
                e.Handled = false;
                return;
            }

            //Point oldpInMm = _oldZone.Px2Mm(_oldPoint);
            Point pInMm = _oldZone.Px2Mm(pIn);
            Zone zoneOut = null;

            var minDx = 0.0;
            var minDy = 0.0;

            if (pIn.Y > _oldZone.Px.Bottom)
            {
                foreach (var zone in _zones.All/*.Where(z => z.Mm.Top > _oldZone.Mm.Bottom)*/)
                {
                    if (zone.Mm.Left > pInMm.X || zone.Mm.Right < pInMm.X) continue;
                    var dy = zone.Mm.Top - _oldZone.Mm.Bottom;

                    if (dy < 0) continue;
                    if (dy > minDy && minDy > 0) continue;
                         
                    
                    // = pInMm + new Vector(0, dy);
                    minDy = dy;
                    zoneOut = zone;
                }
            }
            else if (pIn.Y < _oldZone.Px.Top)
            {
                foreach (var zone in _zones.All/*.Where(z => !ReferenceEquals(z, _oldZone))*/)
                {
                    if (zone.Mm.Left > pInMm.X || zone.Mm.Right < pInMm.X) continue;
                    var dy = zone.Mm.Bottom - _oldZone.Mm.Top;

                    if (dy > 0) continue;
                    if (dy < minDy && minDy < 0) continue;

                    minDy= dy;
                    zoneOut = zone;
                }
            }

            if (pIn.X > _oldZone.Px.Right)
            {
                foreach (var zone in _zones.All)
                {
                    if (zone.Mm.Top > pInMm.Y || zone.Mm.Bottom < pInMm.Y) continue;
                    var dx = zone.Mm.Left - _oldZone.Mm.Right;

                    if (dx < 0) continue;
                    if (dx > minDx && minDx > 0) continue;

                    minDx = dx;
                    zoneOut = zone;
                }
            }
            else if (pIn.X < _oldZone.Px.Left)
            {
                foreach (var zone in _zones.All)
                {
                    if (zone.Mm.Top > pInMm.Y || zone.Mm.Bottom < pInMm.Y) continue;
                    var dx = zone.Mm.Right - _oldZone.Mm.Left;

                    if (dx < minDx && minDx < 0) continue;
                    if (dx > 0) continue;

                    minDx = dx;
                    zoneOut = zone;
                }
            }

            if (zoneOut == null)
            {
                LbmMouse.CursorPos = _oldZone.InsidePx(pIn);
                e.Handled = true;
                return;
            }

            var pOut = zoneOut.Mm2Px(new Point(pInMm.X+minDx,pInMm.Y+minDy));
            pOut = zoneOut.InsidePx(pOut);
            _oldZone = zoneOut.Main;
            _oldPoint = pOut;
            LbmMouse.CursorPos = pOut;
            ZoneChanged?.Invoke(this, new ZoneChangeEventArgs(_oldZone, zoneOut));
            e.Handled = true;
        }

        private void MouseMoveCross(object sender, MouseEventExtArgs e)
        {
            if (e.Clicked) return;
            var pIn = new Point(e.X, e.Y);

            if (_oldZone.ContainsPx(pIn))
            {
                _oldPoint = pIn;
                e.Handled = false;
                return;
            }
            //if (_count >= 0) _timer.Start();
            //try
            //{
            Point oldpInMm = _oldZone.Px2Mm(_oldPoint);
                Point pInMm = _oldZone.Px2Mm(pIn);
                Zone zoneOut = null;

                    var seg = new Segment(oldpInMm, pInMm);
                    var minDist = double.PositiveInfinity;

                    var pOutInMm = pInMm;

                    foreach (var zone in _zones.All.Where(z => !ReferenceEquals(z, _oldZone)))
                    {
                        foreach (var p in seg.Line.Intersect(zone.Mm))
                        {
                            var travel = new Segment(oldpInMm, p);
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
                        LbmMouse.CursorPos = _oldZone.InsidePx(pIn);
                        e.Handled = true;
                        return;
                    }

                    var pOut = zoneOut.Mm2Px(pOutInMm);
                    pOut = zoneOut.InsidePx(pOut);
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

        private void AdjustPointer(object sender, ZoneChangeEventArgs args)
        {
            if (args.NewZone.Dpi - args.OldZone.Dpi < 1) return;
            if (args.NewZone.Dpi > 110)
            {
                    LbmMouse.SetCursorAero(args.NewZone.Dpi > 138 ? 3 : 2);
            }
            else LbmMouse.SetCursorAero(1);            
        }

        private void AdjustSpeed(object sender, ZoneChangeEventArgs args)
        {
            LbmMouse.MouseSpeed = Math.Round((5.0/96.0)* args.NewZone.Dpi, 0);
        }

        private void HomeCinema(object sender, ZoneChangeEventArgs args)
        {
            args.OldZone.Screen.Monitor.Vcp().Power = false;
            args.NewZone.Screen.Monitor.Vcp().Power = true;
        }

 
        public void MatchConfig(string configId)
        {
            Config.MatchConfig(configId);
        }
    }
}

