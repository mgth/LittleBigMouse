/*
  LittleBigMouse.Ui.Avalonia
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

  This file is part of LittleBigMouse.Ui.Avalonia.

    LittleBigMouse.Ui.Avalonia is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    LittleBigMouse.Ui.Avalonia is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/

using System;
using System.IO;
using System.Linq;
using System.Text.Json;
using System.Threading.Tasks;
using System.Windows.Input;
using Avalonia.Controls;
using HLab.Mvvm.Annotations;
using HLab.Mvvm.ReactiveUI;
using LittleBigMouse.Plugins.Localization;
using LittleBigMouse.DisplayLayout.Dimensions;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.DisplayLayout.Monitors.Extensions;
using HLab.Base.ReactiveUI;
using LittleBigMouse.Plugins;
using LittleBigMouse.Plugins.Persistence;
using LittleBigMouse.Ui.Avalonia.Controls;
using LittleBigMouse.Ui.Avalonia.Main;
using LittleBigMouse.Ui.Avalonia.MonitorFrame;
using LittleBigMouse.Ui.Avalonia.Remote;
using LittleBigMouse.Zoning;
using ReactiveUI;

namespace LittleBigMouse.Ui.Avalonia;

public class MonitorsLayoutPresenterViewModel 
    : ViewModel<IMonitorsLayout>
        , IMvvmContextProvider
        , IMonitorsLayoutPresenterViewModel
{
    public IMainPluginsViewModel MainViewModel { get; }

    // Nullable: only the greedy ctor is resolved by the DI container at runtime; the
    // design-mode path never invokes the apply command (same pattern as DefaultMonitorViewModel).
    readonly IDisplayController? _controller;
    readonly AgentClient? _agent;

    public MonitorsLayoutPresenterViewModel(IMainPluginsViewModel mainViewModel)
        : this(mainViewModel, null, null)
    {
    }

    public MonitorsLayoutPresenterViewModel(
        IMainPluginsViewModel mainViewModel,
        IDisplayController? controller,
        AgentClient? agent)
    {
        MainViewModel = mainViewModel;
        _controller = controller;
        _agent = agent;

        // Edge-prober reports ride the hook events the agent forwards. A report describes
        // one loaded layout: a model swap (rebuild, back-to-local) makes it stale, so it
        // is dropped then.
        if (agent is not null)
        {
            EventHandler<LittleBigMouseServiceEventArgs> onHookEvent = (_, args) =>
            {
                if (args.Event == LittleBigMouseEvent.Probed
                    && Zoning.ProbeReport.TryParse(args.Payload, out var report))
                    ProbeReport = report;
            };
            agent.HookEventReceived += onHookEvent;
            Disposer.OnDispose(() => agent.HookEventReceived -= onHookEvent);
        }

        this.WhenAnyValue(e => e.Model)
            .Subscribe(_ => ProbeReport = null)
            .DisposeWith(this);

        _width = this.WhenAnyValue(
                e => e.Model.PhysicalBounds.Width,
                e => e.VisualRatio.X,
                (w, x) => w * x)
            .ToProperty(this, e => e.Width);

        _height = this.WhenAnyValue(
                e => e.Model.PhysicalBounds.Height,
                e => e.VisualRatio.Y,
                (h, y) => h * y)
            .ToProperty(this, e => e.Height);

        ResetLocationsFromSystem = ReactiveCommand.Create(() => Model?.SetLocationsFromSystemConfiguration());
        ApplyLocationsToSystem = ReactiveCommand.CreateFromTask<Window?>(ApplyLocationsToSystemAsync);
    }

    /// <summary>
    /// Push the physical layout to the system display configuration (inverse of
    /// <see cref="MonitorsLocationsFromSystemExtensions.SetLocationsFromSystemConfiguration"/>).
    /// </summary>
    async Task ApplyLocationsToSystemAsync(Window? owner)
    {
        if (Model is not MonitorsLayout layout || _controller is null) return;

        var (confirmed, adjustScale) = await MonitorWarningDialog.ShowApplyLayoutAsync(
            owner, offerScale: OperatingSystem.IsLinux());
        if (!confirmed) return;

        // The system change triggers a rebuild that re-imports the system layout and then
        // loads the saved one: the current physical layout survives only if saved first.
        // The agent writes it (v6: one writer, the store it reads) — and if it cannot, the
        // system is left alone rather than rearranged around a layout about to be lost.
        if (!layout.IsVirtual && !layout.Saved)
        {
            if (await SaveThroughAgentAsync(_agent, layout) is { } refused)
            {
                await MonitorWarningDialog.ShowNotAppliedAsync(owner, refused);
                return;
            }
        }

        // Computed after the save, synchronously up to SetLocations: what is applied is
        // the layout just saved, with no await in between for an edit to slip into.
        // No pre-filtering: the whole topology goes to the controller, which may need to
        // translate it (kscreen refuses negative positions) before it can tell what
        // actually changes.
        var locations = layout.ComputePixelLocationsFromPhysical(adjustScale)
            .Select(kv => (kv.Key, kv.Value.PixelBounds.Location, kv.Value.Scale))
            .ToList();

        if (locations.Count == 0) return;

        if (!_controller.SetLocations(locations))
            await MonitorWarningDialog.ShowFailureAsync(owner, LbmLocalizer.Current.Get("Dialog.MonitorWarning.Failure.Action.ApplyLayout"));
    }

    /// <summary>
    /// The Save button's request (LocationControlViewModel.SaveAsync), for the layout as it
    /// is now. The editor stays usable while the agent answers, so what the agent wrote is
    /// the document SENT: an edit made meanwhile is not in it, and marking the layout saved
    /// — then applying, then reloading the saved document — would drop that edit silently.
    /// </summary>
    /// <returns>Null once the agent has written exactly the current layout, else why not.</returns>
    internal static async Task<string?> SaveThroughAgentAsync(AgentClient? agent, MonitorsLayout layout)
    {
        if (agent is null) return "no agent";
        var sent = AgentDocument.Of(layout);
        try
        {
            await agent.SaveLayoutAsync(layout.Id, sent);
        }
        catch (Exception error) when (error is AgentException
                                      or OperationCanceledException
                                      or IOException)
        {
            var reason = error is OperationCanceledException ? "no agent answered" : error.Message;
            Console.Error.WriteLine($"Save before apply refused: {reason}");
            return reason;
        }

        if (!SameDocument(sent, AgentDocument.Of(layout)))
        {
            Console.Error.WriteLine("Save before apply: the layout changed while it was being saved");
            return "the layout changed while it was being saved";
        }

        LayoutPersistence.MarkLayoutSaved(layout);
        return null;
    }

    static bool SameDocument(AgentDocumentDto a, AgentDocumentDto b)
        => JsonSerializer.Serialize(a) == JsonSerializer.Serialize(b);

    public MonitorsLayoutPresenterViewModel():this(new MainViewModelDesign())
    {
        if(!Design.IsDesignMode) throw new InvalidOperationException("Only for design mode");
    }

    public double Width => _width.Value;
    readonly ObservableAsPropertyHelper<double> _width;

    public double Height => _height.Value;
    readonly ObservableAsPropertyHelper<double> _height;

    public IMutableDisplayRatio VisualRatio { get; } = new DisplayRatioValue(1.0);

    public PhysicalMonitor? SelectedMonitor { get; set => this.RaiseAndSetIfChanged(ref field, value);}

    public Zoning.ProbeReport? ProbeReport
    {
        get;
        private set => this.RaiseAndSetIfChanged(ref field, value);
    }

    public ICommand ResetLocationsFromSystem { get; }
    public ICommand ApplyLocationsToSystem { get; }

    public void ConfigureMvvmContext(IMvvmContext ctx)
    {
        ctx.AddCreator<MonitorFrameViewModel>(vm => vm.MonitorsPresenter = this);
    }
}
