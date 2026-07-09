using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Reactive;
using System.Threading;
using Avalonia;
using Avalonia.Controls;
using Avalonia.Logging;
using ReactiveUI.Avalonia;
using Grace.DependencyInjection;
using HLab.Bugs.Avalonia;
using HLab.Core;
using HLab.Core.Annotations;
using HLab.Icons.Avalonia;
using HLab.Mvvm;
using HLab.Mvvm.Annotations;
using HLab.Mvvm.Avalonia;
using HLab.Sys.Windows.Monitors;
using HLab.Theme.Avalonia;
using HLab.UserNotification;
using HLab.UserNotification.Avalonia;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Plugin.Layout.Avalonia.LocationPlugin;
using LittleBigMouse.Plugin.Vcp.Avalonia;
using LittleBigMouse.Plugins;
using LittleBigMouse.Ui.Avalonia.Main;
using LittleBigMouse.Ui.Avalonia.Plugins.Debug;
using LittleBigMouse.Ui.Avalonia.Remote;
using LittleBigMouse.Ui.Avalonia.Updater;
using LittleBigMouse.Ui.Core;
using ReactiveUI;
using Splat;
using LittleBigMouseClientService = LittleBigMouse.Ui.Avalonia.Remote.LittleBigMouseClientService;

namespace LittleBigMouse.Ui.Avalonia;

internal class Program
{
    const string APP_GUID = "51B5711E-1A7F-436E-B3DD-B598901B3FD2";
    const string SHOW_EVENT_NAME = APP_GUID + "_ShowWindow";

    // Exposed so MainService can wait on it without a DI dependency on Program.
    internal static EventWaitHandle? ShowWindowEvent;

    // Initialization code. Don't use any Avalonia, third-party APIs or any
    // SynchronizationContext-reliant code before AppMain is called: things aren't initialized
    // yet and stuff might break.
    [STAThread]
    public static void Main(string[] args)
    {
        using var mutex = new Mutex(true, APP_GUID);

        if (!mutex.WaitOne(TimeSpan.Zero, false))
        {
            // Signal the running instance to show its window, then exit.
            try
            {
                using var handle = EventWaitHandle.OpenExisting(SHOW_EVENT_NAME);
                handle.Set();
            }
            catch { }
            return;
        }

        // TODO (Avalonia 12 / ReactiveUI 23): RxApp.DefaultExceptionHandler is now the read-only
        // RxState.DefaultExceptionHandler. To restore a custom handler, configure it through
        // RxAppBuilder.CreateReactiveUIBuilder().WithExceptionHandler(...).BuildApp().

        using var showWindowEvent = new EventWaitHandle(false, EventResetMode.AutoReset, SHOW_EVENT_NAME);
        ShowWindowEvent = showWindowEvent;

        BuildAvaloniaApp().Start(UIMain, args);

        ShowWindowEvent = null;
    }

    // Avalonia configuration, don't remove; also used by visual designer.
    public static AppBuilder BuildAvaloniaApp()
    {
        //GC.KeepAlive(typeof(SvgImageExtension).Assembly);
        //GC.KeepAlive(typeof(global::Avalonia.Svg.Skia.Svg).Assembly);

        return AppBuilder.Configure<App>()
            .UseReactiveUI(_ => { })
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

            // TODO (ReactiveUI 23): configure the exception handler via RxAppBuilder.WithExceptionHandler.

#if DEBUG
            Locator.CurrentMutable.RegisterConstant(new LoggingService { Level = LogLevel.Info }, typeof(ILogger));
#endif

            var container = new DependencyInjectionContainer();
            container.Configure(c =>
            {
                //c.ExportInstance(app.ApplicationLifetime);

                c.Export<ApplicationUpdaterViewModel>().As<IApplicationUpdater>().Lifestyle.Singleton();
                c.Export<IconService>().As<IIconService>().Lifestyle.Singleton();
                c.Export<LocalizationService>().As<ILocalizationService>().Lifestyle.Singleton();
                c.Export<MvvmService>().As<IMvvmService>().Lifestyle.Singleton();
                c.ExportInstance<Func<Type, object>>(t => container.Locate(t));
                c.Export<HLab.Core.MessageBus>().As<IMessagesService>().Lifestyle.Singleton();
                c.Export<UserNotificationServiceAvalonia>().As<IUserNotificationService>().Lifestyle.Singleton();

                c.Export<MainService>().As<IMainService>().Lifestyle.Singleton();

                c.Export<SystemMonitorsService>().As<ISystemMonitorsService>().Lifestyle.Singleton();

                // Platform seam: the UI builds its layout through ILayoutFactory and mutates
                // topology through IDisplayController. The Windows implementations live in
                // LittleBigMouse.Platform.Windows (a Linux head would register its own).
                // SystemMonitorsService stays registered for the Windows-only (debug) VCP
                // plugin until its own seam lands later.
                c.Export<LittleBigMouse.Platform.Windows.WindowsLayoutFactory>().As<LittleBigMouse.Plugins.ILayoutFactory>().Lifestyle.Singleton();
                c.Export<LittleBigMouse.Platform.Windows.WindowsDisplayController>().As<LittleBigMouse.Plugins.IDisplayController>().Lifestyle.Singleton();

                c.Export<LittleBigMouseClientService>().As<ILittleBigMouseClientService>().Lifestyle.Singleton();
                c.Export<LbmOptions>().As<ILayoutOptions>().Lifestyle.Singleton();
                c.Export<ProcessesCollector>().As<IProcessesCollector>().Lifestyle.Singleton();

                // A new layout is built on every display change and the previous one is
                // disposed by MainService. Grace must NOT track these transients in its
                // root disposal scope: it would keep every generation reachable until
                // app shutdown (gigabytes after hours of display-event storms).
                c.Export<MonitorsLayout>().ExternallyOwned();

                c.Export<MainViewModel>().As<IMainPluginsViewModel>().Lifestyle.Singleton();

                var parser = new AssemblyParser();

                parser.LoadDll("LittleBigMouse.Ui.Core");
                parser.LoadDll("LittleBigMouse.Plugin.Layout.Avalonia");
                parser.LoadDll("LittleBigMouse.Plugin.Vcp.Avalonia");

                parser.LoadModules();

                // Views and view-models are transient and IDisposable (ReactiveModel). Grace tracks
                // every transient IDisposable it creates in its ROOT disposal scope and holds it until
                // the app closes — so a new generation of monitor VMs on every display-change rebuild
                // is retained forever (gigabytes under a display-event storm), even though HLab.Mvvm
                // disposes them on DetachedFromLogicalTree. Their lifetime is owned by the view tree,
                // NOT the container: mark them ExternallyOwned so Grace stops tracking them (same fix
                // as MonitorsLayout above, #484 — this is the ViewModel follow-up).
                parser.Add<IView>(t => c.Export(t).As(typeof(IView)).ExternallyOwned());
                parser.Add<IViewModel>(t => c.Export(t).As(typeof(IViewModel)).ExternallyOwned());
                parser.Add<Bootloader>(t => c.Export(t).As(typeof(Bootloader)));

                parser.Parse();
            });



            var boot = new Bootstrapper(() => container.Locate<IEnumerable<Bootloader>>());

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