using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using LbmScreenConfig;

namespace LittleBigMouse_Control
{
    public abstract class ScreensPresenter : NotifyUserControl
    {
        public ScreenConfig Config => MainGui.Instance.Config;

        public abstract IEnumerable<ScreenGuiControl> AllControlGuis { get; }

    }
}
