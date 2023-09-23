using System;
using System.Windows;
using System.Windows.Interop;
using HLab.DependencyInjection.Annotations;

namespace HLab.Windows.Monitors
{
    /// <summary>
    /// Logique d'interaction pour DispalyChangesView.xaml
    /// </summary>
    public partial class DispalyChangesView : Window
    {
        private static IMonitorsService _monitorsService;

        [Import]
        public DispalyChangesView(IMonitorsService service)
        {
            _monitorsService = service;
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
        private static IntPtr WndProc(IntPtr hwnd, int msg, IntPtr wParam, IntPtr lParam, ref bool handled)
        {
            var message = (WindowMessage)msg;
            var subCode = (WindowMessageParameter)wParam.ToInt64();
            if (message == WindowMessage.WM_WININICHANGE)
            {
                //MonitorsService.D.UpdateDevices();
            }

            if (message == WindowMessage.WM_DISPLAYCHANGE /*|| message == WindowMessage.WM_WININICHANGE*/)
            {
                _monitorsService.UpdateDevices();
            }

            return IntPtr.Zero;
        }

    }
}
