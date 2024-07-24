namespace LittleBigMouse.Plugins;

public interface IApplicationUpdater
{
    Task CheckUpdateAsync(bool show);
}