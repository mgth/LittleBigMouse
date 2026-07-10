using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Reactive;
using System.Threading;
using System.Threading.Tasks;
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
    // Exposed so MainService can subscribe to ShowRequested without a DI dependency on Program.
    internal static SingleInstanceGuard? SingleInstance;

    // Initialization code. Don't use any Avalonia, third-party APIs or any
    // SynchronizationContext-reliant code before AppMain is called: things aren't initialized
    // yet and stuff might break.
    [STAThread]
    public static void Main(string[] args)
    {
        // Null means another instance already runs (and was signaled to show its window).
        using var instance = SingleInstanceGuard.TryAcquire();
        if (instance == null) return;

        // TODO (Avalonia 12 / ReactiveUI 23): RxApp.DefaultExceptionHandler is now the read-only
        // RxState.DefaultExceptionHandler. To restore a custom handler, configure it through
        // RxAppBuilder.CreateReactiveUIBuilder().WithExceptionHandler(...).BuildApp().

        SingleInstance = instance;

        BuildAvaloniaApp().Start(UIMain, args);

        SingleInstance = null;
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

                // SystemMonitorsService stays registered on every OS: its constructor is inert
                // (the Win32 enumeration is lazy behind .Root) and view-models inject it, so
                // Grace must be able to resolve it even on Linux where .Root is never touched.
                c.Export<SystemMonitorsService>().As<ISystemMonitorsService>().Lifestyle.Singleton();

                // Platform seam: the UI builds its layout through ILayoutFactory and mutates
                // topology through IDisplayController. Implementations live in
                // LittleBigMouse.Platform.Windows / LittleBigMouse.Platform.Linux.
                if (OperatingSystem.IsWindows())
                {
                    c.Export<LittleBigMouse.Platform.Windows.WindowsLayoutFactory>().As<LittleBigMouse.Plugins.ILayoutFactory>().Lifestyle.Singleton();
                    c.Export<LittleBigMouse.Platform.Windows.WindowsDisplayController>().As<LittleBigMouse.Plugins.IDisplayController>().Lifestyle.Singleton();
                    c.Export<LittleBigMouse.Platform.Windows.WindowsLayoutPersistence>().As<LittleBigMouse.Plugins.ILayoutPersistence>().Lifestyle.Singleton();
                }
                else
                {
                    // The controller injects the concrete factory (NotifyDisplayChanged):
                    // export it under both the seam interface and its own type.
                    c.Export<LittleBigMouse.Platform.Linux.LinuxLayoutFactory>()
                        .As<LittleBigMouse.Plugins.ILayoutFactory>()
                        .As<LittleBigMouse.Platform.Linux.LinuxLayoutFactory>()
                        .Lifestyle.Singleton();
                    c.Export<LittleBigMouse.Platform.Linux.LinuxDisplayController>().As<LittleBigMouse.Plugins.IDisplayController>().Lifestyle.Singleton();
                    c.Export<LittleBigMouse.Platform.Linux.LinuxLayoutPersistence>().As<LittleBigMouse.Plugins.ILayoutPersistence>().Lifestyle.Singleton();
                }

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
                // VCP talks DDC/CI through Win32 (and its view-models walk the Win32 device
                // tree): Windows-only until it gets its own platform seam.
                if (OperatingSystem.IsWindows())
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

            // Theming is fully variant-based: RequestedThemeVariant="Default" (App.axaml)
            // follows the OS on every platform and the HLab.* resources are merged as
            // ThemeDictionaries — no manual ThemeService dictionary swap anymore.

            var cts = new CancellationTokenSource();

            var task = boot.BootAsync();

            // A bootloader failure would otherwise stay invisible until shutdown (the task is
            // only awaited after app.Run returns) — leaving the app alive with no window and
            // no tray icon, and nothing to diagnose it with.
            task.ContinueWith(
                t => Console.Error.WriteLine($"Boot failed: {t.Exception?.Flatten().InnerException}"),
                TaskContinuationOptions.OnlyOnFaulted);

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