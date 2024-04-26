namespace LittleBigMouse.Ui.Avalonia.Controls;

public class ListItem
{
    public ListItem(string id, string caption, string description)
    {
        Id = id;
        Caption = caption;
        Description = description;
    }

    public string Id { get; }
    public string Caption { get; }
    public string Description { get; }
}