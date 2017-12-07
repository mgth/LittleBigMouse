using Hlab.Mvvm;
using Hlab.Notify;

namespace LittleBigMouse.LocationPlugin.Plugins.Location.Rulers
{
    public class TesterViewModel : ViewModel
    {
        public double LeftInDip
        {
            get => this.Get(() => default(double));
            set => this.Set(value);
        }

        public double RightInDip
        {
            get => this.Get(() => default(double));
            set => this.Set(value);
        }

        public double TopInDip
        {
            get => this.Get(() => default(double));
            set => this.Set(value);
        }

        public double BottomInDip
        {
            get => this.Get(() => default(double));
            set => this.Set(value);
        }
        public double HeightInDip
        {
            get => this.Get(() => default(double));
            set => this.Set(value);
        }
        public double WidthInDip
        {
            get => this.Get(() => default(double));
            set => this.Set(value);
        }
    }
}
