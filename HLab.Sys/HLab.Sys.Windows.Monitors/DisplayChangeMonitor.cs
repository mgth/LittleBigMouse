using System;

namespace HLab.Sys.Windows.Monitors;

public class DisplayChangeMonitor
{
    public event EventHandler<EventArgs> DisplayChanged; 
    public void Hook()
    {
        //TODO 
        // hwndSource.AddHook(WndProc);
    }

    IntPtr WndProc(IntPtr hwnd, int msg, IntPtr wParam, IntPtr lParam, ref bool handled)
    {
        var message = (WindowMessage)msg;
        var subCode = (WindowMessageParameter)wParam.ToInt64();
        if (message == WindowMessage.WM_WININICHANGE)
        {
            //MonitorsService.D.UpdateDevices();
        }

        if (message == WindowMessage.WM_DISPLAYCHANGE /*|| message == WindowMessage.WM_WININICHANGE*/)
        {
            DisplayChanged?.Invoke(this,EventArgs.Empty);
        }

        return 0;
    }

}