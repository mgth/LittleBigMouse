namespace LittleBigMouse.Plugin.Layout.Avalonia.BorderResistancePlugin;

/// <summary>
/// One monitor edge, drawn at true size on the real screen and editable there.
/// <para>
/// It hosts the same <see cref="BorderSideStrip"/> as the layout map, so the two
/// views cannot disagree and neither has to be refreshed by hand: the strip reads
/// the section collection directly, and derives its millimetre scale from its own
/// size — which, inside this window, is the screen edge itself.
/// </para>
/// </summary>
public partial class ScreenSectionsWindow : EdgeOverlayWindow
{
    public ScreenSectionsWindow()
    {
        InitializeComponent();
    }

    public ScreenSectionsWindow(BorderSideViewModel viewModel) : this()
    {
        DataContext = viewModel;

        // These windows come and go with the toggle, and their view models hold
        // subscriptions to the monitor's shared section list.
        Closed += (_, _) => viewModel.Dispose();
    }
}
