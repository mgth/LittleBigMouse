using System;
using System.Collections.Generic;
using Avalonia;
using Avalonia.Markup.Xaml;
using Grace.DependencyInjection;
using HLab.Core;
using HLab.Core.Annotations;
using HLab.Icons.Annotations.Icons;
using HLab.Icons.Avalonia;
using HLab.Mvvm;
using HLab.Mvvm.Annotations;
using HLab.Mvvm.Avalonia;
using HLab.Notify.Annotations;
using HLab.Notify.Avalonia;
using HLab.Sys.Windows.Monitors;
using HLab.UserNotification;
using HLab.UserNotification.Avalonia;
using LittleBigMouse.DisplayLayout;
using LittleBigMouse.Plugins;
using LittleBigMouse.Ui.Avalonia.Main;
using LittleBigMouse.Ui.Core;
using Splat;

namespace LittleBigMouse.Ui.Avalonia;

public partial class App : Application
{
    public override void Initialize()
    {
        AvaloniaXamlLoader.Load(this);
    }

    public override void OnFrameworkInitializationCompleted()
    {

#if DEBUG
        Locator.CurrentMutable.RegisterConstant(new LoggingService { Level = LogLevel.Info }, typeof(ILogger));
#endif


        var container = new DependencyInjectionContainer();
        container.Configure(c =>
        {
            c.Export<EventHandlerServiceAvalonia>().As<IEventHandlerService>().Lifestyle.Singleton();


            //NotifyHelper.EventHandlerService = new EventHandlerServiceAvalonia();

            c.Export<IconService>().As<IIconService>().Lifestyle.Singleton();
            c.Export<LocalizationService>().As<ILocalizationService>().Lifestyle.Singleton();
            c.Export<MvvmService>().As<IMvvmService>().Lifestyle.Singleton();
            c.Export<MvvmAvaloniaImpl>().As<IMvvmPlatformImpl>().Lifestyle.Singleton();
            c.Export<MessageBus>().As<IMessagesService>().Lifestyle.Singleton();
            c.Export<UserNotificationServiceAvalonia>().As<IUserNotificationService>().Lifestyle.Singleton();

            c.Export<MainService>().As<IMainService>().Lifestyle.Singleton();
            c.Export<MonitorsService>().As<IMonitorsService>().Lifestyle.Singleton();

            c.Export<MainBootloader>().As<IBootloader>();

            c.Export<LittleBigMouseClientService>().As<ILittleBigMouseClientService>().Lifestyle.Singleton();

            c.Export<Layout>().As<IMonitorsLayout>();

            c.Export<MainViewModel>().As<IMainPluginsViewModel>().Lifestyle.Singleton();

            var parser = new AssemblyParser();

            //parser.LoadDll("LittleBigMouse.Plugin.Location");
            //parser.LoadDll("LittleBigMouse.Plugin.Vcp");

            parser.LoadModules();

            parser.Add<IView>(t => c.Export(t).As(typeof(IView)));
            parser.Add<IViewModel>(t => c.Export(t).As(typeof(IViewModel)));
            parser.Add<IBootloader>(t => c.Export(t).As(typeof(IBootloader)));

            parser.Parse();
        });

        var boot = new Bootstrapper(() => container.Locate<IEnumerable<IBootloader>>());

        //var theme = new ThemeService(Resources);
        //theme.SetTheme(ThemeService.WindowsTheme.Dark);

        boot.Boot();

        //if (ApplicationLifetime is IClassicDesktopStyleApplicationLifetime desktop)
        //{
        //    desktop.MainWindow = new MainView().AsWindow();
        //}

        base.OnFrameworkInitializationCompleted();




    }

    public class LoggingService : ILogger
    {
        public void Write(Exception exception, string message, Type type, LogLevel logLevel)
        {
            if (logLevel >= Level)
                System.Diagnostics.Debug.WriteLine(message);
        }

        public LogLevel Level { get; set; }

        public void Write(string message, LogLevel logLevel)
        {
            if (logLevel >= Level)
                System.Diagnostics.Debug.WriteLine(message);
        }

        public void Write(Exception exception, string message, LogLevel logLevel)
        {
            if (logLevel >= Level)
                System.Diagnostics.Debug.WriteLine(message);
        }

        public void Write(string message, Type type, LogLevel logLevel)
        {
            if (logLevel >= Level)
                System.Diagnostics.Debug.WriteLine(message);
        }
    }

}