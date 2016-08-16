using System.Windows.Controls;

namespace LittleBigMouse_Control
{
    internal class ScreenControlViewModel : ScreenViewModel
    {
        public ScreenFrameViewModel Frame
        {
            get { return GetProperty<ScreenFrameViewModel>(); }
            set { SetProperty(value); }
        }

        public Grid CoverControl { get; } = new Grid();
    }
}
