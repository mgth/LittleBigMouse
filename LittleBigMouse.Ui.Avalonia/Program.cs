using System;
using Avalonia;
using Avalonia.ReactiveUI;
using Avalonia.Svg;
using Avalonia.Svg.Skia;

namespace LittleBigMouse.Ui.Avalonia;

internal class Program
{
    // Initialization code. Don't use any Avalonia, third-party APIs or any
    // SynchronizationContext-reliant code before AppMain is called: things aren't initialized
    // yet and stuff might break.
    [STAThread]
    public static void Main(string[] args) => BuildAvaloniaApp()
        .StartWithClassicDesktopLifetime(args);

    // Avalonia configuration, don't remove; also used by visual designer.
    public static AppBuilder BuildAvaloniaApp()
    {
        GC.KeepAlive(typeof(SvgImageExtension).Assembly);
        GC.KeepAlive(typeof(global::Avalonia.Svg.Skia.Svg).Assembly);

        return AppBuilder.Configure<App>()
            .UseReactiveUI()
            .UsePlatformDetect()
            .UseSkia()
            .LogToTrace();
    }
}