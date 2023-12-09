using Avalonia;
using Avalonia.Markup.Xaml;

namespace LittleBigMouse.Ui.Avalonia;

public partial class App : Application
{
    public override void Initialize()
    {
        AvaloniaXamlLoader.Load(this);
    }

    public override void OnFrameworkInitializationCompleted()
    {
        Program.UIMain(this, new string[] { });
    }
}