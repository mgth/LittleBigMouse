using System.Windows;
using Hlab.Mvvm;
using Hlab.Plugin;

namespace LittleBigMouse_Control
{
    /// <summary>
    /// Logique d'interaction pour App.xaml
    /// </summary>
    public partial class App : Application
    {
        protected override void OnStartup(StartupEventArgs e)
        {
            base.OnStartup(e);

            PluginService.D.Register();
        }
    }

}

