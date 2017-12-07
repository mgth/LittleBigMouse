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
using System.Windows.Forms;

namespace LittleBigMouse_Daemon
{
    public class Notify //: IDisposable
    {
        private readonly System.Windows.Forms.NotifyIcon _notify;

        public Notify()
        {
            _notify = new NotifyIcon
            {
                Visible = true,
                ContextMenuStrip = new ContextMenuStrip(),
                Icon = Properties.Resources.lbm_off
        };

            SetOff();
            _notify.MouseClick += _notify_MouseClick;

         }

        public void SetOn()
        {
            _notify.Icon = Properties.Resources.lbm2_on;
        }
        public void SetOff()
        {
            _notify.Icon = Properties.Resources.lbm_off;
        }
        public void Show()
        {
            _notify.Visible = true;
        }
        public void Hide()
        {
            _notify.Visible = false;
        }

        private void _notify_MouseClick(object sender, MouseEventArgs e)
        {
            if (e.Button == MouseButtons.Right)
                _notify.ContextMenuStrip.Show( Control.MousePosition);

            if (e.Button == MouseButtons.Left)
                Click?.Invoke(sender, e);
        }


        public event EventHandler Click;
        //public void Dispose()
        //{
        //    Dispose(true);
        //    GC.SuppressFinalize(this);
        //}
        public delegate void Func();

        public void AddMenu(int pos, string txt, EventHandler evt, string tag=null, bool chk = false)
        {
            ToolStripMenuItem item = new ToolStripMenuItem(txt, null, evt);

            item.Checked = chk;

            item.Tag = tag;

            if(pos<0 || pos>= _notify.ContextMenuStrip.Items.Count) _notify.ContextMenuStrip.Items.Add(item);

            else _notify.ContextMenuStrip.Items.Insert(pos,item);
                
        }

        public void RemoveMenu(string tag)
        {
            bool done = false;
            while (!done)
            {
                ToolStripItem[] items = new ToolStripItem[_notify.ContextMenuStrip.Items.Count];
                _notify.ContextMenuStrip.Items.CopyTo(items,0);
                done = true;
                foreach (ToolStripItem i in items)
                {
                    if (i.Tag as string == tag)
                    {
                        _notify.ContextMenuStrip.Items.Remove(i);
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
