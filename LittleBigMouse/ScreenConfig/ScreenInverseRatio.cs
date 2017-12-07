using Hlab.Notify;

namespace LbmScreenConfig
{
    public static class ScreenInverseRatioExt
    {
        public static ScreenRatio Inverse(this ScreenRatio source) => new ScreenInverseRatio(source);
    }
    public class ScreenInverseRatio : ScreenRatio
    {
        public ScreenRatio Source
        {
            get => this.Get<ScreenRatio>();
            private set => this.Set(value);
        }

        public ScreenInverseRatio(ScreenRatio ratio)
        {
            Source = ratio;
            this.Subscribe();
        }

        [TriggedOn(nameof(Source), "X")]
        public override double X
        {
            get => this.Get(()=> 1/Source.X);
            set => Source.X = 1/value;
        }

        [TriggedOn(nameof(Source), "Y")]
        public override double Y
        {
            get => this.Get(()=> 1/Source.Y);
            set => Source.Y = 1/value;
        }
    }
}