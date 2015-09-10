/*
  MouseControl - Mouse Managment in multi DPI monitors environment
  Copyright (c) 2015 Mathieu GRENET.  All right reserved.

  This file is part of MouseControl.

    ArduixPL is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    ArduixPL is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/

using System;

namespace LittleBigMouse
{
    public class Notify : IDisposable
    {
        private System.Windows.Forms.NotifyIcon _notify;

        public Notify()
        {
            _notify = new System.Windows.Forms.NotifyIcon
            {
                Icon = Properties.Resources.MainIcon,
                Visible = true,
            };

            _notify.Click +=
                delegate (object sender, EventArgs args)
                {
                    if (Click!=null) Click(sender,args);
                };
        }

        public event EventHandler Click;
        public void Dispose()
        {
            Dispose(true);
            GC.SuppressFinalize(this);
        }
        protected virtual void Dispose(bool disposing)
        {
            if (disposing)
            {
                _notify.Visible = false;
                _notify.Dispose();
            }
        }
    }
}
