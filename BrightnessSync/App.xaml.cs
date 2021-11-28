using System;
using System.Collections.Generic;
using System.Configuration;
using System.Data;
using System.Linq;
using System.Threading.Tasks;
using System.Windows;

namespace BrightnessSync
{
    /// <summary>
    /// Interaction logic for App.xaml
    /// </summary>
    public partial class App : Application
    {
    }

    /*
    private readonly LuminanceWindow _brightness = new LuminanceWindow();
    private void OnNotifyClick(object sender, EventArgs e) { Brightness(); }

    public void Brightness(object sender, EventArgs eventArgs)
    {
        Brightness();
    }
    private void Brightness()
    {
        if (_brightness == null) return;

        if (_brightness.Visibility == Visibility.Visible)
            _brightness.Hide();
        else
        {
            _brightness.Hook = _engine.Hook;
            _brightness.Show();
            
        }
    }
    */


}
