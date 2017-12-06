using LbmScreenConfig;

namespace LittleBigMouse_Control.Plugins
{
    internal interface IPluginButton
    {
        string Caption { get; }
        bool IsActivated { get; set; }
    }

}
