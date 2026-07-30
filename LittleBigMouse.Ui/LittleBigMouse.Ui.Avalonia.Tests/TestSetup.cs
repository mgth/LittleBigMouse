using System.Runtime.CompilerServices;
using ReactiveUI.Builder;

namespace LittleBigMouse.Ui.Avalonia.Tests;

static class TestSetup
{
    // ReactiveUI 23 refuses WhenAnyValue before the builder ran; the app gets this
    // through Avalonia's UseReactiveUI, tests need the core services only. Same
    // initializer as LittleBigMouse.DisplayLayout.Tests.
    [ModuleInitializer]
    internal static void InitReactiveUI()
        => RxAppBuilder.CreateReactiveUIBuilder()
            .WithCoreServices()
            .BuildApp();
}
