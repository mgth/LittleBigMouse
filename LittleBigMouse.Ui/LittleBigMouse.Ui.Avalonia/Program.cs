using System;
using System.Collections.Generic;
using System.Reactive;
using System.Runtime.ExceptionServices;
using System.Threading;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Logging;
using Avalonia.ReactiveUI;
using Grace.DependencyInjection;
using HLab.Base.Avalonia.Themes;
using HLab.Bugs.Avalonia;
using HLab.Core;
using HLab.Core.Annotations;
using HLab.Icons.Avalonia;
using HLab.Mvvm;
using HLab.Mvvm.Annotations;
using HLab.Mvvm.Avalonia;
using HLab.Sys.Windows.Monitors;
using HLab.UserNotification;
using HLab.UserNotification.Avalonia;
using LittleBigMouse.DisplayLayout;
using LittleBigMouse.Plugin.Layout.Avalonia.LocationPlugin;
using LittleBigMouse.Plugin.Vcp.Avalonia;
using LittleBigMouse.Plugins;
using LittleBigMouse.Ui.Avalonia.Main;
using LittleBigMouse.Ui.Avalonia.Plugins.Debug;
using LittleBigMouse.Ui.Core;
using ReactiveUI;
using Splat;

namespace LittleBigMouse.Ui.Avalonia;

internal class Program
{
    // Initialization code. Don't use any Avalonia, third-party APIs or any
    // SynchronizationContext-reliant code before AppMain is called: things aren't initialized
    // yet and stuff might break.
    [STAThread]
    public static void Main(string[] args) => BuildAvaloniaApp()
        //.StartWithClassicDesktopLifetime(args,ShutdownMode.OnExplicitShutdown);
        .Start(UIMain, args);

    // Avalonia configuration, don't remove; also used by visual designer.
    public static AppBuilder BuildAvaloniaApp()
    {
        //GC.KeepAlive(typeof(SvgImageExtension).Assembly);
        //GC.KeepAlive(typeof(global::Avalonia.Svg.Skia.Svg).Assembly);

        return AppBuilder.Configure<App>()
            .UseReactiveUI()
            .UsePlatformDetect()
            .WithInterFont()
            //.With(new Win32PlatformOptions
            //{
            //    RenderingMode = [Win32RenderingMode.AngleEgl],
            //    CompositionMode = [Win32CompositionMode.WinUIComposition]

            //})
            .UseSkia()
#if DEBUG
            //            .LogToTrace(LogEventLevel.Verbose)
            .LogToTrace(LogEventLevel.Debug)
#endif
            ;
    }

    public static void UIMain(Application app, string[] args)
    {
        // Avalonia is already initialized here, so it's safe to spawn the thread for console Main
        if (Design.IsDesignMode)
        {
            //InitDesignMode();
            return;
        }

        try
        {

            RxApp.DefaultExceptionHandler = Observer.Create<Exception>(Console.WriteLine);

#if DEBUG
            Locator.CurrentMutable.RegisterConstant(new LoggingService { Level = LogLevel.Info }, typeof(ILogger));
#endif

            var container = new DependencyInjectionContainer();
            container.Configure(c =>
            {
                //c.ExportInstance(app.ApplicationLifetime);

                c.Export<IconService>().As<IIconService>().Lifestyle.Singleton();
                c.Export<LocalizationService>().As<ILocalizationService>().Lifestyle.Singleton();
                c.Export<MvvmService>().As<IMvvmService>().Lifestyle.Singleton();
                c.Export<MvvmAvaloniaImpl>().As<IMvvmPlatformImpl>().Lifestyle.Singleton();
                c.Export<HLab.Core.MessageBus>().As<IMessagesService>().Lifestyle.Singleton();
                c.Export<UserNotificationServiceAvalonia>().As<IUserNotificationService>().Lifestyle.Singleton();

                c.Export<MainService>().As<IMainService>().Lifestyle.Singleton();

                c.Export<SystemMonitorsService>().As<ISystemMonitorsService>().Lifestyle.Singleton();
                c.Export<MainBootloader>().As<IBootloader>();

                c.Export<LittleBigMouseClientService>().As<ILittleBigMouseClientService>().Lifestyle.Singleton();

                //c.Export<MonitorsLayout>().As<IMonitorsLayout>();

                c.Export<MainViewModel>().As<IMainPluginsViewModel>().Lifestyle.Singleton();

                c.Export<MonitorDebugPlugin>().As<IBootloader>();
                c.Export<MonitorLocationPlugin>().As<IBootloader>();

                c.Export<VcpPlugin>().As<IBootloader>();


                var parser = new AssemblyParser();

                parser.LoadDll("LittleBigMouse.Plugin.Layout.Avalonia");
                parser.LoadDll("LittleBigMouse.Plugin.Vcp.Avalonia");

                parser.LoadModules();

                parser.Add<IView>(t => c.Export(t).As(typeof(IView)));
                parser.Add<IViewModel>(t => c.Export(t).As(typeof(IViewModel)));
                parser.Add<IBootloader>(t => c.Export(t).As(typeof(IBootloader)));

                parser.Parse();
            });

            var boot = new Bootstrapper(() => container.Locate<IEnumerable<IBootloader>>());

            var theme = new ThemeService(app.Resources);
            theme.SetTheme(ThemeService.WindowsTheme.Auto);

            var cts = new CancellationTokenSource();

            var task = boot.BootAsync();

            try
            {
                app.Run(cts.Token);
            }
            catch (InvalidOperationException)
            {
                cts.Cancel();
            }

            try
            {
                task.Wait(cts.Token);
            }
            catch (OperationCanceledException)
            {
            }
        }
        catch (Exception ex)
        {
            var view = new ExceptionView
            {
                Exception = ex,
                ProductHeaderValue = "LittleBigMouse",
                Project = "LittleBigMouse",
                Repository = "Mgth"
            };
            view.Show();
            //throw;
            //ExceptionDispatchInfo.Capture(ex).Throw();
        }

    }
}