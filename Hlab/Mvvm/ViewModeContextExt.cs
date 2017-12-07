using System.Windows;

namespace Hlab.Mvvm
{
    public static class ViewModeContextExt
    {
        public static ViewModeContext GetViewModeContext(this UIElement element)
        {
            return element.GetValue(ViewLocator.ViewModeContextProperty) as ViewModeContext;
        }
    }
}
