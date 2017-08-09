using System.Windows.Controls;
using Erp.Notify;

namespace LittleBigMouse_Control
{
    internal class ScreenControlViewModel : ScreenViewModel
    {
        public ScreenFrameViewModel Frame
        {
            get => this.Get<ScreenFrameViewModel>(); set => this.Set(value);
        }

        public Grid CoverControl { get; } = new Grid();
    }
}
