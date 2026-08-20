using System;
using System.Reactive.Disposables;
using System.Threading.Tasks;
using Avalonia.Controls;
using HLab.Mvvm;
using HLab.Mvvm.Annotations;
using HLab.Mvvm.Avalonia;
using LittleBigMouse.Plugins;

namespace LittleBigMouse.Ui.Avalonia.Main;

/// <summary>
/// The configuration window: building it on demand, bringing back the one that already exists,
/// and letting it go when it closes.
/// <para>
/// Closing is not leaving — the layout lives on the service and comes back with the window —
/// but it is the gesture that reads as "done", so it is also the moment to say the
/// configuration was never saved. Avalonia gives no way to ask a question mid-close, so the
/// close is cancelled, the question asked, and the close repeated once answered.
/// </para>
/// </summary>
public sealed class MainWindowManager(
    IMvvmService mvvmService,
    Func<IMainPluginsViewModel> viewModelLocator) : IDisposable
{
    Action<IMainPluginsViewModel>? _plugins;
    Window? _window;

    /// <summary>
    /// Handlers on the live window. They die with the window either way, but naming them makes
    /// "the window is gone and so is everything watching it" a statement the code makes rather
    /// than one the garbage collector eventually agrees with.
    /// </summary>
    CompositeDisposable? _windowSubscriptions;

    /// <summary>The open window, or null when there is none.</summary>
    public Window? Current => _window;

    /// <summary>Contribute to the window's view-model when it is next built.</summary>
    public void AddPlugin(Action<IMainPluginsViewModel>? action) => _plugins += action;

    /// <summary>
    /// Show the configuration window: restore the existing one, or build, place and open a new one.
    /// </summary>
    /// <param name="mainService">Handed to the view-model, which drives the layout through it.</param>
    /// <param name="confirmClose">
    /// Asked before the window actually closes. False means the user chose to stay.
    /// </param>
    public void Show(IMainService mainService, Func<Window, Task<bool>> confirmClose)
    {
        if (_window?.IsLoaded == true)
        {
            _window.WindowState = WindowState.Normal;
            _window.Activate();
            return;
        }

        var viewModel = viewModelLocator();
        viewModel.MainService = mainService;

        _plugins?.Invoke(viewModel);

        var view = mvvmService
            .MainContext
            .GetView<DefaultViewMode>(viewModel, typeof(IDefaultViewClass));

        var window = view?.AsWindow() ?? throw new Exception("No window found");

        // AsWindow() creates a bare DefaultWindow with no size: restore the last
        // session's geometry (or a sensible default) and save it back on close.
        MainWindowGeometry.Attach(window);

        var subscriptions = new CompositeDisposable();
        var closeConfirmed = false;

        void OnClosed(object? sender, EventArgs e)
        {
            _window = null;
            ReleaseWindowSubscriptions();
        }

        async void OnClosing(object? sender, WindowClosingEventArgs e)
        {
            if (closeConfirmed || sender is not Window closing) return;

            e.Cancel = true;
            if (!await confirmClose(closing)) return;

            closeConfirmed = true;
            closing.Close();
        }

        subscriptions.Add(OwnedSubscription.Create<EventHandler>(
            OnClosed, h => window.Closed += h, h => window.Closed -= h));
        subscriptions.Add(OwnedSubscription.Create<EventHandler<WindowClosingEventArgs>>(
            OnClosing, h => window.Closing += h, h => window.Closing -= h));

        // Let go of the window being replaced before adopting this one. There is a narrow way to
        // get here with one still outstanding — a second show landing before the first window
        // reports IsLoaded — and its Closed handler would otherwise fire later and null out
        // _window, leaving the open window unreachable and the next show building a third.
        ReleaseWindowSubscriptions();
        _windowSubscriptions = subscriptions;
        _window = window;

        window.Show();
    }

    void ReleaseWindowSubscriptions()
    {
        _windowSubscriptions?.Dispose();
        _windowSubscriptions = null;
    }

    /// <summary>
    /// Release what watches the window. The window itself is left alone: tearing it down here
    /// would make disposing the service a visible event for the user, which it is not.
    /// <para>
    /// This does drop the unsaved-changes prompt, so anything disposing the manager is
    /// committing to the edits being lost. Only shutdown does, and shutdown asks the question
    /// itself, on its way out.
    /// </para>
    /// </summary>
    public void Dispose() => ReleaseWindowSubscriptions();
}
