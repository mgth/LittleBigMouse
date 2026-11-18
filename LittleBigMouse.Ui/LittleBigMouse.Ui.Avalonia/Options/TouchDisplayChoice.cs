using System;
using ReactiveUI;

namespace LittleBigMouse.Ui.Avalonia.Options;

public sealed class TouchDisplayChoice : ReactiveObject
{
    readonly Action<string, bool> _changed;
    readonly string _id;
    bool _selected;
    public string Label { get; }

    public TouchDisplayChoice(string id, string label, bool selected, Action<string, bool> changed)
    {
        _id = id;
        Label = label;
        _selected = selected;
        _changed = changed;
    }

    public bool Selected
    {
        get => _selected;
        set
        {
            if (_selected == value) return;
            this.RaiseAndSetIfChanged(ref _selected, value);
            _changed(_id, value);
        }
    }
}
