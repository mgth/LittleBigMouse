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
using Microsoft.Extensions.DependencyInjection;
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
using LittleBigMouse.Plugin.Vcp.Avalonia.SamsungTizen;
using LittleBigMouse.Plugin.Vcp.Avalonia.HisenseVidaa;
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
        StartupLog.RedirectConsoleWhenDetached();

        // Null means another instance already runs (and was signaled to show its window).
        using var instance = SingleInstanceGuard.TryAcquire();
        if (instance == null) return;

        // TODO (Avalonia 12 / ReactiveUI 23): RxApp.DefaultExceptionHandler is now the read-only
        // RxState.DefaultExceptionHandler. To restore a custom handler, configure it through
        // RxAppBuilder.CreateReactiveUIBuilder().WithExceptionHandler(...).BuildApp().

        SingleInstance = instance;

        try
        {
            BuildAvaloniaApp().Start(UIMain, args);
        }
        catch (Exception ex)
        {
            // Avalonia platform init happens before UIMain's own handler exists.
            Console.Error.WriteLine($"Fatal: {ex}");
            throw;
        }

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

            var services = new ServiceCollection();

            services.AddSingleton<IApplicationUpdater, ApplicationUpdaterViewModel>();
            services.AddSingleton<IIconService, IconService>();
            services.AddSingleton<ILocalizationService, LocalizationService>();
            services.AddSingleton<IMvvmService, MvvmService>();
            services.AddSingleton<IMessagesService, HLab.Core.MessageBus>();
            services.AddSingleton<IUserNotificationService, UserNotificationServiceAvalonia>();

            services.AddSingleton<IMainService, MainService>();

            // The MVVM locator: registered services come from the container, anything
            // else (views, view-models, MonitorsLayout…) is built by ActivatorUtilities.
            // Instances created that way are transient and NOT tracked by the root
            // disposal scope — the ExternallyOwned semantics this app relies on: a new
            // generation of monitor VMs is built on every display-change rebuild and
            // disposed by HLab.Mvvm on DetachedFromLogicalTree; tracking them in the
            // container would retain every generation until shutdown (gigabytes after
            // hours of display-event storms, #484).
            services.AddSingleton<Func<Type, object>>(sp =>
                t => sp.GetService(t) ?? ActivatorUtilities.CreateInstance(sp, t));

            // MS.DI does not synthesize Func<T> factories like Grace did: the ones the
            // app injects are registered explicitly.
            services.AddSingleton<Func<MonitorsLayout>>(sp =>
                () => ActivatorUtilities.CreateInstance<MonitorsLayout>(sp));
            services.AddSingleton<Func<IMainPluginsViewModel>>(sp =>
                () => sp.GetRequiredService<IMainPluginsViewModel>());
            services.AddSingleton<Func<ApplicationUpdaterViewModel>>(sp =>
                () => ActivatorUtilities.CreateInstance<ApplicationUpdaterViewModel>(sp));
            services.AddSingleton<Func<VcpScreenViewModel, LittleBigMouse.Plugin.Vcp.Avalonia.Patterns.TestPatternButtonViewModel>>(sp =>
                vm => ActivatorUtilities.CreateInstance<LittleBigMouse.Plugin.Vcp.Avalonia.Patterns.TestPatternButtonViewModel>(sp, vm));

            // SystemMonitorsService stays registered on every OS: its constructor is inert
            // (the Win32 enumeration is lazy behind .Root) and view-models inject it, so
            // the container must be able to resolve it even on Linux where .Root is never touched.
            services.AddSingleton<ISystemMonitorsService, SystemMonitorsService>();

            // Platform seam: the UI builds its layout through ILayoutFactory and mutates
            // topology through IDisplayController. Implementations live in
            // LittleBigMouse.Platform.Windows / LittleBigMouse.Platform.Linux.
            if (OperatingSystem.IsWindows())
            {
                services.AddSingleton<LittleBigMouse.Plugins.ILayoutFactory, LittleBigMouse.Platform.Windows.WindowsLayoutFactory>();
                services.AddSingleton<LittleBigMouse.Plugins.IDisplayController, LittleBigMouse.Platform.Windows.WindowsDisplayController>();
                services.AddSingleton<LittleBigMouse.Plugins.ILayoutPersistence, LittleBigMouse.Platform.Windows.WindowsLayoutPersistence>();
                services.AddSingleton<LittleBigMouse.Plugins.IMonitorInfoService, LittleBigMouse.Platform.Windows.WindowsMonitorInfoService>();
                services.AddSingleton<LittleBigMouse.Plugins.IWallpaperService, LittleBigMouse.Platform.Windows.WindowsWallpaperService>();
                services.AddSingleton<IVcpService, WindowsVcpService>();
            }
            else
            {
                // The controller injects the concrete factory (NotifyDisplayChanged):
                // register it under both the seam interface and its own type.
                services.AddSingleton<LittleBigMouse.Platform.Linux.LinuxLayoutFactory>();
                services.AddSingleton<LittleBigMouse.Plugins.ILayoutFactory>(sp =>
                    sp.GetRequiredService<LittleBigMouse.Platform.Linux.LinuxLayoutFactory>());
                services.AddSingleton<LittleBigMouse.Plugins.IDisplayController, LittleBigMouse.Platform.Linux.LinuxDisplayController>();
                services.AddSingleton<LittleBigMouse.Plugins.ILayoutPersistence, LittleBigMouse.Platform.Linux.LinuxLayoutPersistence>();
                services.AddSingleton<LittleBigMouse.Plugins.IMonitorInfoService, LittleBigMouse.Platform.Linux.LinuxMonitorInfoService>();
                services.AddSingleton<LittleBigMouse.Plugins.IWallpaperService, LittleBigMouse.Platform.Linux.PlasmaWallpaperService>();
                services.AddSingleton<IVcpService, DdcUtilVcpService>();
            }

            // Samsung smart monitors (notably the Odyssey G80SD) expose a local
            // Tizen remote-control channel even when they provide no DDC/CI VCP.
            services.AddSingleton<SamsungTizenSettingsStore>();
            services.AddSingleton<ISamsungTizenService, SamsungTizenService>();
            services.AddSingleton<HisenseVidaaSettingsStore>();
            services.AddSingleton<IHisenseVidaaService, HisenseVidaaService>();

            services.AddSingleton<ILittleBigMouseClientService, LittleBigMouseClientService>();
            services.AddSingleton<ILayoutOptions, LbmOptions>();
            services.AddSingleton<IProcessesCollector, ProcessesCollector>();

            services.AddSingleton<IMainPluginsViewModel, MainViewModel>();

            var parser = new AssemblyParser();

            parser.LoadDll("LittleBigMouse.Ui.Core");
            parser.LoadDll("LittleBigMouse.Plugin.Layout.Avalonia");
            // VCP goes through the IVcpService seam: dxva2 on Windows, ddcutil on
            // Linux (monitors without a reachable DDC/CI channel just get no sliders).
            parser.LoadDll("LittleBigMouse.Plugin.Vcp.Avalonia");
            // Wallpaper drives the desktop through IWallpaperService (plasmashell
            // scripting on Linux, IDesktopWallpaper COM on Windows); the plugin
            // hides itself where IsSupported is false (GNOME, bare X11…).
            parser.LoadDll("LittleBigMouse.Plugin.Wallpaper.Avalonia");
            services.AddSingleton<LittleBigMouse.Plugin.Wallpaper.Avalonia.WallpaperManager>();

            parser.LoadModules();

            // Views and view-models are NOT registered: the MVVM locator builds them
            // with ActivatorUtilities (see above). Only bootloaders need discovery.
            parser.Add<Bootloader>(t => services.AddTransient(typeof(Bootloader), t));

            parser.Parse();

            var container = services.BuildServiceProvider();

            var boot = new Bootstrapper(() => container.GetServices<Bootloader>());

            // Theming is fully variant-based: RequestedThemeVariant="Default" (App.axaml)
            // follows the OS on every platform and the HLab.* resources are merged as
            // ThemeDictionaries — no manual ThemeService dictionary swap anymore.

            var cts = new CancellationTokenSource();

            // An exception in a UI event handler or a posted job would otherwise unwind
            // app.Run and silently kill the window (only ExceptionView remains). Log it
            // and keep the dispatcher alive — a broken click must not take the app down.
            global::Avalonia.Threading.Dispatcher.UIThread.UnhandledException += (_, args) =>
            {
                Console.Error.WriteLine($"Unhandled UI exception: {args.Exception}");
                args.Handled = true;
            };

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
            Console.Error.WriteLine($"Fatal startup exception: {ex}");
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
