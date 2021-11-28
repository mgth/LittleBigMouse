using System.Windows;
using HLab.Notify.PropertyChanged;

namespace LittleBigMouse.ScreenConfig.Dimensions
{
    using H=H<ScreenRatioValue>;

    public class ScreenRatioValue : ScreenRatio
    {
        public ScreenRatioValue(double x, double y)
        {
            H.Initialize(this);
            X = x;
            Y = y;
        }
        public ScreenRatioValue(double r)
        {
            H.Initialize(this);
            X = r;
            Y = r;
        }
        public ScreenRatioValue(Vector v)
        {
            H.Initialize(this);
            X = v.X;
            Y = v.Y;
        }

        public override double X
        {
            get => _x.Get();
            set => _x.Set(value);
        }
        private readonly IProperty<double> _x = H.Property<double>();

        public override double Y
        {
            get => _y.Get();
            set => _y.Set(value);
        }
        private readonly IProperty<double> _y = H.Property<double>(); 
    }
}