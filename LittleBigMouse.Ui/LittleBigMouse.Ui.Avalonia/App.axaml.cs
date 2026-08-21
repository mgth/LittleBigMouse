using Avalonia;
using Avalonia.Markup.Xaml;

namespace LittleBigMouse.Ui.Avalonia;

public partial class App : Application
{
    public override void Initialize()
    {
        AvaloniaXamlLoader.Load(this);
    }

    // No OnFrameworkInitializationCompleted override, on purpose. Program.Main runs the
    // application through `BuildAvaloniaApp().Start(UIMain, args)`, and Start does Setup()
    // *then* invokes the delegate — so overriding this to call Program.UIMain, as this class
    // used to, wired the entry point up twice.
    //
    // The second call is invisible while the app is alive, because the first one is still
    // parked in app.Run inside Setup(). It only lands on the way out: app.Run returns at
    // shutdown, the override returns, Setup() returns, and Start then calls UIMain again —
    // which rebuilds the whole application over a dead dispatcher. A second ServiceCollection,
    // a second container, a second LittleBigMouseClientService with its own IPC listener; that
    // listener finds no daemon (the Quit just took it down), asks for one to be launched, and
    // the daemon it spawns outlives the process. That was the orphaned `lbm-hook` after every
    // Exit. The second app.Run then throws "the Dispatcher shut down" and the process exits.
    //
    // Running UIMain from Start alone also hands it the real command line rather than the
    // empty array this override passed.
}