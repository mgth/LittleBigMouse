using System.Collections.Generic;
using HLab.Mvvm.Annotations;
using LittleBigMouse.Ui.Avalonia.Options;

namespace LittleBigMouse.Ui.Avalonia.Controls;

/// <summary>
/// Design-time stand-in for <see cref="LocationControlViewModel"/>, used only by
/// <c>LocationControlView.axaml</c>'s <c>Design.DataContext</c>.
/// <para>
/// It used to carry an <c>AlgorithmList</c> and a <c>SelectedAlgorithm</c>, left behind when
/// the crossing-algorithm picker moved to the options page. Nothing bound them: the view
/// compiles its bindings against <see cref="LocationControlViewModel"/>
/// (<c>x:CompileBindings="True"</c>), which declares neither, and the view has no Algorithm
/// binding at all. They were a fourth spelling of a value that already had too many — see
/// <c>wire-contract/README.md</c> — so they are gone rather than kept in step.
/// </para>
/// </summary>
public class LocationControlViewModelDesign : IDesignViewModel
{
}