using System;

namespace LittleBigMouse_Control.VcpPlugin
{
    class VcpControlViewModel : ViewModel
    {
        public override Type ViewType => typeof (Plugins.Vcp.VcpControlView);
    }
}
