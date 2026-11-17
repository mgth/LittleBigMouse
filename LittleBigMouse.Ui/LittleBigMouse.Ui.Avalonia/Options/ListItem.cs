using ReactiveUI;

namespace LittleBigMouse.Ui.Avalonia.Options;

public class ListItem : ReactiveObject
{
    public ListItem(string id, string caption, string description)
    {
        Id = id;
        _caption = caption;
        _description = description;
    }

    public string Id { get; }

    public string Caption
    {
        get => _caption;
        internal set => this.RaiseAndSetIfChanged(ref _caption, value);
    }
    string _caption;

    public string Description
    {
        get => _description;
        internal set => this.RaiseAndSetIfChanged(ref _description, value);
    }
    string _description;
}
