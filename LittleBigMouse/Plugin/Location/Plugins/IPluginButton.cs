namespace LittleBigMouse.LocationPlugin.Plugins
{
    internal interface IPluginButton
    {
        string Caption { get; }
        bool IsActivated { get; set; }
    }

}
