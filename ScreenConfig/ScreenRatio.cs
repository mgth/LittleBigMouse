using System;
using System.ComponentModel;
using Erp.Notify;

namespace LbmScreenConfig
{
    public abstract class ScreenRatio : INotifyPropertyChanged
    {
        public event PropertyChangedEventHandler PropertyChanged
        {
            add => this.Add(value);
            remove => this.Remove(value);
        }

        public abstract double X { get; set; }
        public abstract double Y { get; set; }

    }

    public class ScreenRatioValue : ScreenRatio
    {
        public ScreenRatioValue(double x, double y)
        {
            X = x;
            Y = y;
            this.Subscribe();
        }
        public ScreenRatioValue(double r)
        {
            X = r;
            Y = r;
            this.Subscribe();
        }

        public override double X
        {
            get => this.Get<double>();
            set => this.Set(value);
        }

        public override double Y
        {
            get => this.Get<double>();
            set => this.Set(value);
        }
    }
    public static class ScreenRatioRatioExt
    {
        public static ScreenRatio Multiply(this ScreenRatio sourceA,ScreenRatio sourceB) => new ScreenRatioRatio(sourceA,sourceB);
    }
    public class ScreenRatioRatio : ScreenRatio
    {
        public ScreenRatio SourceA
        {
            get => this.Get<ScreenRatio>();
            private set => this.Set(value);
        }
        public ScreenRatio SourceB
        {
            get => this.Get<ScreenRatio>();
            private set => this.Set(value);
        }

        public ScreenRatioRatio(ScreenRatio ratioA, ScreenRatio ratioB)
        {
            using (this.Suspend())
            {
                SourceA = ratioA;
                SourceB = ratioB;
            }
        }

        [TriggedOn(nameof(SourceA), "X")]
        [TriggedOn(nameof(SourceB), "X")]
        public override double X
        {
            get => this.Get(() => SourceA.X * SourceB.X);
            set => throw new NotImplementedException();
        }

        [TriggedOn(nameof(SourceA), "Y")]
        [TriggedOn(nameof(SourceB), "Y")]
        public override double Y
        {
            get => this.Get(() => SourceA.Y * SourceB.Y);
            set => throw new NotImplementedException();
        }
    }
}