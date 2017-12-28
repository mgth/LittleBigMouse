using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Data;
using System.Windows.Documents;
using System.Windows.Input;
using System.Windows.Interop;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using System.Windows.Navigation;
using System.Windows.Shapes;

namespace HLab.Windows.Monitors
{
    /// <summary>
    /// Logique d'interaction pour DispalyChangesView.xaml
    /// </summary>
    public partial class DispalyChangesView : Window
    {
        public DispalyChangesView()
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
        private static IntPtr WndProc(IntPtr hwnd, int msg, IntPtr wParam, IntPtr lParam, ref bool handled)
        {
            var message = (WindowMessage)msg;
            var subCode = (WindowMessageParameter)wParam.ToInt32();

            if (message == WindowMessage.WM_DISPLAYCHANGE /*|| message == WindowMessage.WM_WININICHANGE*/)
            {
                MonitorsService.D.UpdateDevices();
            }

            return IntPtr.Zero;
        }
    }
}
