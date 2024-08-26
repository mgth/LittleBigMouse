using HLab.Base.ReactiveUI;
using ReactiveUI;

namespace LittleBigMouse.Ui.Avalonia.Options;

public class SeenProcessViewModel : ReactiveModel
{
    public SeenProcessViewModel(string caption, string description, LbmOptionsViewModel options)
    {
        Caption = caption;
        Description = description;
        _match = options.WhenAnyValue(
            e => e.Pattern,
            e =>
            {
                if (string.IsNullOrWhiteSpace(e)) return false;
                return caption.Contains(e);
            }).ToProperty(this, e => e.Match);
    }

    public string Caption { get; }
    public string Description { get; }
    public bool Match => _match.Value;
    readonly ObservableAsPropertyHelper<bool> _match;
}