namespace LittleBigMouse.Ui.Avalonia.Options;

public class ListItem(string id, string caption, string description)
{
    public string Id { get; } = id;
    public string Caption { get; } = caption;
    public string Description { get; } = description;
}