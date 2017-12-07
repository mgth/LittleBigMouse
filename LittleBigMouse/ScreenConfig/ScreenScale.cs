using Hlab.Notify;

namespace LbmScreenConfig
{
    public static class ScreenScaleExt
    {
        public static ScreenSize Scale(this ScreenSize source, ScreenRatio ratio) => new ScreenScale(source, ratio);
    }
    public class ScreenScale : ScreenSize
    {
        public ScreenScale(ScreenSize source, ScreenRatio ratio)
        {
            using (this.Suspend())
            {
                Source = source;
                Ratio = ratio;
            }
            this.Subscribe();
        }

        public ScreenRatio Ratio
        {
            get => this.Get<ScreenRatio>();
            set => this.Set(value);
        }

        [TriggedOn(nameof(Source), "Width")]
        [TriggedOn(nameof(Ratio), "X")]
        public override double Width
        {
            get => this.Get(() => Source.Width * Ratio.X);
            set => Source.Width = value / Ratio.X;
        }

        [TriggedOn(nameof(Source), "Height")]
        [TriggedOn(nameof(Ratio), "Y")]
        public override double Height
        {
            get => this.Get(() => Source.Height * Ratio.Y);
            set => Source.Height = value / Ratio.Y;
        }

        [TriggedOn(nameof(Source), "X")]
        [TriggedOn(nameof(Ratio), "X")]
        public override double X
        {
            get => this.Get(() => Source.X /** Ratio.X*/);
            set => Source.X = value /*/ Ratio.X*/;
        }

        [TriggedOn(nameof(Source), "Y")]
        [TriggedOn(nameof(Ratio), "Y")]
        public override double Y
        {
            get => this.Get(() => Source.Y /** Ratio.Y*/);
            set => Source.Y = value /*/ Ratio.Y*/;
        }

        [TriggedOn(nameof(Source), "TopBorder")]
        [TriggedOn(nameof(Ratio), "Y")]
        public override double TopBorder
        {
            get => this.Get(() => Source.TopBorder * Ratio.Y);
            set => Source.TopBorder = value / Ratio.Y;
        }

        [TriggedOn(nameof(Source), "BottomBorder")]
        [TriggedOn(nameof(Ratio), "Y")]
        public override double BottomBorder
        {
            get => this.Get(() => Source.BottomBorder * Ratio.Y);
            set => Source.BottomBorder = value / Ratio.Y;
        }

        [TriggedOn(nameof(Source), "LeftBorder")]
        [TriggedOn(nameof(Ratio), "X")]
        public override double LeftBorder
        {
            get => this.Get(() => Source.LeftBorder * Ratio.X);
            set => Source.LeftBorder = value / Ratio.X;
        }

        [TriggedOn(nameof(Source), "RightBorder")]
        [TriggedOn(nameof(Ratio), "X")]
        public override double RightBorder
        {
            get => this.Get(() => Source.RightBorder * Ratio.X);
            set => Source.RightBorder = value / Ratio.X;
        }
    }
}