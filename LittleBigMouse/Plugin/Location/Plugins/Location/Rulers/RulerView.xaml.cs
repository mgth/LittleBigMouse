/*
  LittleBigMouse.Plugin.Location
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

  This file is part of LittleBigMouse.Plugin.Location.

    LittleBigMouse.Plugin.Location is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    LittleBigMouse.Plugin.Location is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/

using System.Windows;
using System.Windows.Controls;
using System.Windows.Input;
using System.Windows.Interop;
using HLab.Mvvm.Annotations;
using HLab.Windows.API;
using LittleBigMouse.ScreenConfigs;

namespace LittleBigMouse.Plugin.Location.Plugins.Location.Rulers
{
    /// <summary>
    /// Logique d'interaction pour Sizer.xaml
    /// </summary>
    /// 


 
    public partial class RulerView : UserControl, IView<RulerViewModel>
    {
        public RulerView()
        {
            InitializeComponent();
        }

        public RulerViewModel ViewModel => DataContext as RulerViewModel;

        private Point _oldPoint;
        private Point _dragStartPoint;

        private void Window_MouseMove(object sender, MouseEventArgs e)
        {
            if (!Moving || _dragStartPoint == null) return;

            Point newPoint = PointToScreen(e.GetPosition(this)); // LbmMouse.CursorPos;
            //newPoint.Offset(
            //    _screen.Config.BoundsInMm.X/DrawOn.PitchX, 
            //    _screen.Config.BoundsInMm.Y/DrawOn.PitchY
            //    );

            if (ViewModel.Vertical)
            {
                double offset = (newPoint.Y - _oldPoint.Y)* ViewModel.DrawOn.Pitch.Y;

                double old = ViewModel.DrawOn.InMm.Y;

                ViewModel.DrawOn.InMm.Y = _dragStartPoint.Y - offset;

                if (ViewModel.DrawOn.Primary && ViewModel.DrawOn.InMm.Y == old) _oldPoint.Y += offset / ViewModel.DrawOn.Pitch.Y;
            }
            else
            {
                double old = ViewModel.DrawOn.InMm.Y;

                double offset = (newPoint.X - _oldPoint.X)* ViewModel.DrawOn.Pitch.X;

                ViewModel.DrawOn.InMm.X = _dragStartPoint.X - offset;

                if (ViewModel.DrawOn.Primary && ViewModel.DrawOn.InMm.X == old) _oldPoint.X += offset / ViewModel.DrawOn.Pitch.X;
            }
        }

        private void Window_MouseDown(object sender, MouseButtonEventArgs e)
        {
            _oldPoint = PointToScreen(e.GetPosition(this));
            //_oldPoint.Offset(_screen.Config.BoundsInMm.X/DrawOn.PitchX, _screen.Config.BoundsInMm.Y/DrawOn.PitchY);

            _dragStartPoint = InvertControl? ViewModel.DrawOn.InMm.Location: ViewModel.Screen.InMm.Location;
            Moving = true;
            CaptureMouse();
        }

        private void Window_MouseUp(object sender, MouseButtonEventArgs e)
        {
            if (Moving)
            {
                Moving = false;
                ReleaseMouseCapture();
            }
        }

        private bool Moving { get; set; } = false;
        public bool InvertControl { get; set; } = true;

        private void Ruler_OnMouseWheel(object sender, MouseWheelEventArgs e)
        {
            double ratio = (e.Delta > 0) ? 1.005 : 1/1.005;

            Point p = e.GetPosition(this);

            //Point pos = new DipPoint(ViewModel.DrawOn.Config, ViewModel.DrawOn, p.X,p.Y).Mm.ToScreen(ViewModel.DrawOn);
            Point pos = ViewModel.DrawOn.InMm.GetPoint(ViewModel.DrawOn.InDip,p);

            if (ViewModel.Vertical)
            {
                ViewModel.DrawOn.PhysicalRatio.Y *= ratio;

                Point pos2 =
                    ViewModel.DrawOn.InMm.GetPoint(ViewModel.DrawOn.InDip, p);

                ViewModel.DrawOn.InMm.Y += pos.Y - pos2.Y;
            }
            else
            {
                ViewModel.DrawOn.PhysicalRatio.X *= ratio;

                Point pos2 =
                    ViewModel.DrawOn.InMm.GetPoint(ViewModel.DrawOn.InDip, p);

                ViewModel.DrawOn.InMm.X += pos.X - pos2.X;
            }
         }

        public void SuspendDrawing()
        {
            var source = (HwndSource)PresentationSource.FromVisual(this);
            if(source!=null)
            NativeMethods.SendMessage(source.Handle, NativeMethods.WM_SETREDRAW, false, 0);
        }

        public void ResumeDrawing()
        {
            var source = (HwndSource)PresentationSource.FromVisual(this);
            if(source!=null)
            NativeMethods.SendMessage(source.Handle, NativeMethods.WM_SETREDRAW, true, 0);
        }

    }
}
