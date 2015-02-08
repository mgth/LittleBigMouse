using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;

namespace MouseControl
{
    public class Notify
    {
        private System.Windows.Forms.NotifyIcon _notify = new System.Windows.Forms.NotifyIcon
        {
            Icon = Properties.Resources.MainIcon,
            Visible = true,
        }; 

        public Notify()
        {
            _notify.Click +=
                delegate (object sender, EventArgs args)
                {
                    if (Click!=null) Click(sender,args);
                };
        }

        public event EventHandler Click;
    }
}
