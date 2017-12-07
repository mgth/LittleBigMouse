namespace Hlab.Mvvm
{
    public abstract class ViewMode { }
    public class ViewModeDetail : ViewMode { }
    public class ViewModeEdit : ViewMode { }
    public class ViewModeSummary : ViewMode { }
    public class ViewModeString : ViewMode { }
    public class ViewModeDefault : ViewMode { }
    public class ViewModeList : ViewMode { }
    public class ViewModePreview : ViewMode { }
    //public class ViewModeDocument : ViewMode { }
    public class ViewModeDraggable : ViewMode { }

    public interface IViewClass { }
    public interface IViewClassDefault  : IViewClass{ }


    public interface IView<TViewMode,TViewModel> 
        where TViewMode : ViewMode
       // where TViewModel : INotifyPropertyChanged
    {
    }
}
