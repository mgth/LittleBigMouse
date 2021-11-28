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
using System.Windows;
using System.Windows.Controls;
using Hardcodet.Wpf.TaskbarNotification;
using LittleBigMouse.Daemon.Properties;

namespace LittleBigMouse.Daemon
{
    public class Notify //: IDisposable
    {
        private readonly TaskbarIcon _notify;

        public Notify()
        {
            _notify = new TaskbarIcon()
            {
                Visibility = Visibility.Visible,
                ContextMenu = new ContextMenu(),
                //Icon = (Icon) Application.Current.FindResource("lbm_off")
            };

            SetOff();

            _notify.TrayLeftMouseUp += _notify_TrayLeftMouseUp; ;
            _notify.TrayRightMouseUp += _notify_TrayRightMouseUp;
        }


        public void SetOn()
        {
            _notify.Icon = Resources.lbm_on;//(Icon) Application.Current.FindResource("lbm_on");
        }
        public void SetOff()
        {
            _notify.Icon = Resources.lbm_off;
            //_notify.Icon = (Icon) Application.Current.FindResource("lbm_off");
        }
        public void Show()
        {
            _notify.Visibility = Visibility.Visible;
        }
        public void Hide()
        {
            _notify.Visibility = Visibility.Hidden;
        }

        private void _notify_TrayRightMouseUp(object sender, RoutedEventArgs e)
        {
            //_notify.ContextMenuStrip.Show( Control.MousePosition);
        }

        private void _notify_TrayLeftMouseUp(object sender, RoutedEventArgs e)
        {
            Click?.Invoke(sender, e);
        }


        public event EventHandler Click;
        //public void Dispose()
        //{
        //    Dispose(true);
        //    GC.SuppressFinalize(this);
        //}
        public delegate void Func();

        public void AddMenu(int pos, string txt, RoutedEventHandler evt, string tag = null, bool chk = false)
        {
            MenuItem item = new MenuItem { };//{ })(txt, null, evt);

            item.Click += evt;


            item.Header = txt;
            item.IsChecked = chk;

            item.Tag = tag;

            if (pos < 0 || pos >= _notify.ContextMenu.Items.Count) _notify.ContextMenu.Items.Add(item);

            else _notify.ContextMenu.Items.Insert(pos, item);

        }

        public void RemoveMenu(string tag)
        {
            bool done = false;
            while (!done)
            {
                MenuItem[] items = new MenuItem[_notify.ContextMenu.Items.Count];
                _notify.ContextMenu.Items.CopyTo(items, 0);
                done = true;
                foreach (var i in items)
                {
                    if (i.Tag as string == tag)
                    {
                        _notify.ContextMenu.Items.Remove(i);
                        done = false;
                    }
                }
            }
        }

        //protected virtual void Dispose(bool disposing)
        //{
        //    if (!disposing) return;

        //    _notify.Visible = false;
        //    _notify.Dispose();
        //}
    }
}
