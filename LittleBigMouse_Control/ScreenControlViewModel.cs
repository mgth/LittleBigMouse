using System.Windows.Controls;

namespace LittleBigMouse_Control
{
    internal class ScreenControlViewModel : ScreenViewModel
    {
        private ScreenFrameViewModel _frame;
        public ScreenFrameViewModel Frame
        {
            get { return _frame; }
            set { SetProperty(ref _frame, value); }
        }

        public Grid CoverControl { get; } = new Grid();
    }
}
