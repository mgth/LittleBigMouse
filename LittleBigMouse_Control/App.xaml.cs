using System;
using System.ServiceModel.Description;
using System.Windows;
using System.Windows.Media;
using Erp.Base;
using Plugin;

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

