using System.Collections.Generic;
using HLab.Mvvm.Annotations;
using LittleBigMouse.Ui.Avalonia.Options;

namespace LittleBigMouse.Ui.Avalonia.Controls;

public class LocationControlViewModelDesign : IDesignViewModel
{
    // Ids match LbmOptionsViewModel.AlgorithmList exactly, case included: they are the wire
    // values the daemon matches on, and SelectedAlgorithm binds by Id.
    public List<ListItem> AlgorithmList { get; } = new()
    {
        new ("Strait","Strait","Simple and highly CPU-efficient transition."),
        new ("Cross","Corner crossing","In direction-friendly manner, allows traversal through corners."),

    };

    public object SelectedAlgorithm { get; set; } = "Strait";
}