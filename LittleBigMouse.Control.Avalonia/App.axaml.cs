using System.Collections.Generic;
using Avalonia;
using Avalonia.Controls.ApplicationLifetimes;
using Avalonia.Markup.Xaml;
using Grace.DependencyInjection;
using HLab.Core;
using HLab.Core.Annotations;
using HLab.Icons.Annotations.Icons;
using HLab.Icons.Avalonia;
using HLab.Mvvm.Annotations;
using HLab.Mvvm.Avalonia;
using HLab.Notify.Annotations;
using HLab.Notify.PropertyChanged;
using HLab.Notify.Wpf;
using HLab.Sys.Windows.Monitors;
using LittleBigMouse.Control.Main;
using LittleBigMouse.DisplayLayout;

namespace LittleBigMouse.Control.Avalonia
{
    public partial class App : Application
    {
        public override void Initialize()
        {
            AvaloniaXamlLoader.Load(this);
        }

        public override void OnFrameworkInitializationCompleted()
        {


            var container = new DependencyInjectionContainer();
            container.Configure(c =>
            {
                c.Export<EventHandlerServiceAvalonia>().As<IEventHandlerService>().Lifestyle.Singleton();
                NotifyHelper.EventHandlerService = new EventHandlerServiceAvalonia();

                c.Export<IconService>().As<IIconService>().Lifestyle.Singleton();
                c.Export<MvvmServiceAvalonia>().As<IMvvmService>().Lifestyle.Singleton();
                c.Export<MessagesService>().As<IMessagesService>().Lifestyle.Singleton();

                c.Export<MainService>().As<IMainService>().Lifestyle.Singleton();
                c.Export<MonitorsService>().As<IMonitorsService>().Lifestyle.Singleton();
                c.Export<LittleBigMouseClientService>().As<ILittleBigMouseClientService>().Lifestyle.Singleton();

                c.Export<Layout>().As<IMonitorsLayout>();

                var parser = new AssemblyParser();

                parser.LoadDll("LittleBigMouse.Plugin.Location");
                parser.LoadDll("LittleBigMouse.Plugin.Vcp");

                parser.LoadModules();

                parser.Add<IView>(t => c.Export(t).As(typeof(IView)));
                parser.Add<IViewModel>(t => c.Export(t).As(typeof(IViewModel)));
                parser.Add<IBootloader>(t => c.Export(t).As(typeof(IBootloader)));

                parser.Parse();
            });

            var boot = new Bootstrapper(() => container.Locate<IEnumerable<IBootloader>>());

            boot.Boot();

            if (ApplicationLifetime is IClassicDesktopStyleApplicationLifetime desktop)
            {
                desktop.MainWindow = new MainView();
            }

            base.OnFrameworkInitializationCompleted();




        }
    }
}