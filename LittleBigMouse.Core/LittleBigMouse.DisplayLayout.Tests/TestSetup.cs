using System.Runtime.CompilerServices;
using ReactiveUI.Builder;

namespace LittleBigMouse.DisplayLayout.Tests;

static class TestSetup
{
    // ReactiveUI 23 refuses WhenAnyValue before the builder ran; the app gets this
    // through Avalonia's UseReactiveUI, tests need the core services only.
    [ModuleInitializer]
    internal static void InitReactiveUI()
        => RxAppBuilder.CreateReactiveUIBuilder()
            .WithCoreServices()
            .BuildApp();
}
