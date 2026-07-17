/*
  LittleBigMouse.Control.Core
  Copyright (c) 2021 Mathieu GRENET.  All right reserved.

  This file is part of LittleBigMouse.Control.Core.

    LittleBigMouse.Control.Core is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    LittleBigMouse.Control.Core is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/

using Avalonia.Controls;
using HLab.Mvvm.ReactiveUI;
using LittleBigMouse.Plugins;
using LittleBigMouse.DisplayLayout.Monitors;
using LittleBigMouse.Ui.Avalonia.Controls;
using LittleBigMouse.Platform.Windows;
using ReactiveUI;
using System;
using System.Reactive.Linq;
using System.Threading.Tasks;
using System.Windows.Input;

namespace LittleBigMouse.Ui.Avalonia.Plugins.Default;

public class DefaultMonitorViewModel : ViewModel<PhysicalMonitor>
{
    // Nullable: the parameterless ctor exists only for XAML/design instantiation, where
    // the attach/detach/primary commands are never invoked. At runtime the MVVM locator
    // resolves the greedier (IDisplayController) ctor via the DI container.
    readonly IDisplayController? _controller;

    public DefaultMonitorViewModel() : this(null) { }

    public DefaultMonitorViewModel(IDisplayController? controller)
    {
        _controller = controller;

        _inches = this.WhenAnyValue(
                e => e.Model.Diagonal,
                selector: d => (d / 25.4).ToString("##.#") + "\"")
            .ToProperty(this, _ => _.Inches);

        _attached = this.WhenAnyValue(e => e.Model.ActiveSource.Source.AttachedToDesktop)
            .ToProperty(this, e => e.Attached);

        _primary = this.WhenAnyValue(e => e.Model.ActiveSource.Source.Primary)
            .ToProperty(this, e => e.Primary);

        _detachVisible = this.WhenAnyValue(
            e => e.Attached,
            e => e.Primary,
            (attached, primary) => attached && !primary)
            .ToProperty(this, e => e.DetachVisible);

        DetachCommand = ReactiveCommand.CreateFromTask<Window?>(DetachFromDesktopAsync,this.WhenAnyValue(
            e => e.Attached,
            e => e.Primary,
            (attached,primary) => attached && !primary)
        .ObserveOn(RxSchedulers.MainThreadScheduler));

        AttachCommand = ReactiveCommand.CreateFromTask<Window?>(AttachToDesktopAsync,this.WhenAnyValue(e => e.Attached, e => !e).ObserveOn(RxSchedulers.MainThreadScheduler));

        MakePrimaryCommand = ReactiveCommand.CreateFromTask<Window?>(MakePrimaryAsync,this.WhenAnyValue(
            e => e.Attached,
            e => e.Primary,
            (attached,primary) => attached && !primary)
        .ObserveOn(RxSchedulers.MainThreadScheduler));

        _excluded = this.WhenAnyValue(e => e.Model.ExcludedFromLayout)
            .ToProperty(this, e => e.Excluded);

        // pure model toggle (no OS action): applied with the layout on save
        ToggleExcludedCommand = ReactiveCommand.Create(
            () => { Model.ExcludedFromLayout = !Model.ExcludedFromLayout; });
    }

    public bool Attached => _attached.Value;
    readonly ObservableAsPropertyHelper<bool> _attached;

    public bool DetachVisible => _detachVisible.Value;
    readonly ObservableAsPropertyHelper<bool> _detachVisible;

    public bool Primary => _primary.Value;
    readonly ObservableAsPropertyHelper<bool> _primary;

    public string Inches => _inches.Value;
    readonly ObservableAsPropertyHelper<string> _inches;

    /// <summary>Monitor kept out of the mouse layout (no zone, cursor-proof) (#504).</summary>
    public bool Excluded => _excluded.Value;
    readonly ObservableAsPropertyHelper<bool> _excluded;

    public ICommand AttachCommand { get; }
    public ICommand DetachCommand { get; }
    public ICommand MakePrimaryCommand { get; }
    public ICommand ToggleExcludedCommand { get; }

    async Task DetachFromDesktopAsync(Window? owner)
    {
        if (!await ConfirmAsync(owner, MonitorWarningDialog.ShowDetachAsync)) return;

        _controller?.DetachFromDesktop(Model.ActiveSource.Source);
    }

    async Task AttachToDesktopAsync(Window? owner)
    {
        if (!await ConfirmAsync(owner, MonitorWarningDialog.ShowAttachAsync)) return;

        _controller?.AttachToDesktop(Model.ActiveSource.Source);
    }

    async Task MakePrimaryAsync(Window? owner)
    {
        if (!await ConfirmAsync(owner, MonitorWarningDialog.ShowMakePrimaryAsync)) return;

        _controller?.SetPrimary(Model.ActiveSource.Source);
    }

    async Task<bool> ConfirmAsync(Window? owner, Func<Window?, Task<(bool Confirmed, bool DontShowAgain)>> showDialog)
    {
        var options = Model.Layout.Options;

        if (!options.ShowMonitorActionWarning) return true;

        var (confirmed, dontShowAgain) = await showDialog(owner);

        if (confirmed && dontShowAgain)
        {
            // persisted immediately by the live-save subscription in MainService
            options.ShowMonitorActionWarning = false;
        }

        return confirmed;
    }

 }