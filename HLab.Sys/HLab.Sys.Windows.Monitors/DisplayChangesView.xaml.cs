using System;
using System.Windows;
using System.Windows.Interop;

namespace HLab.Sys.Windows.Monitors
{
    // <summary>
    // Logique d'interaction pour DispalyChangesView.xaml
    // </summary>
    public partial class DisplayChangesView : Window
    {
        public event EventHandler<EventArgs> DisplayChanged; 


        public DisplayChangesView()
        {
            InitializeComponent();
        }
        protected override void OnSourceInitialized(EventArgs e)
        {
            base.OnSourceInitialized(e);

            if (PresentationSource.FromVisual(this) is HwndSource hwndSource)
            {
                hwndSource.AddHook(WndProc);
            }
        }
        private IntPtr WndProc(IntPtr hwnd, int msg, IntPtr wParam, IntPtr lParam, ref bool handled)
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

            return IntPtr.Zero;
        }

    }
}
