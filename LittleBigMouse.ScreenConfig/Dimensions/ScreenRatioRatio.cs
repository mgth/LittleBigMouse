using System;
using HLab.Notify.PropertyChanged;

namespace LittleBigMouse.ScreenConfig.Dimensions
{
    using H = H<ScreenRatioRatio>;

    public class ScreenRatioRatio : ScreenRatio
    {
        public ScreenRatioRatio(IScreenRatio ratioA, IScreenRatio ratioB)
        {
            SourceA = ratioA;
            SourceB = ratioB;
            H.Initialize(this);
        }

        public IScreenRatio SourceA { get; }
        public IScreenRatio SourceB { get; }

        public override double X
        {
            get => _x.Get();
            set => throw new NotImplementedException();
        }
        private readonly IProperty<double> _x = H.Property<double>(c => c
            .Set(s => s.SourceA.X * s.SourceB.X)
            .On(e => e.SourceA.X)
            .On(e => e.SourceB.X)
            .Update()
        );

        public override double Y
        {
            get => _y.Get();
            set => throw new NotImplementedException();
        }
        private readonly IProperty<double> _y = H.Property<double>(c => c
            .Set(s => s.SourceA.Y * s.SourceB.Y)
            .On(e => e.SourceA.Y)
            .On(e => e.SourceB.Y)
            .Update()
        );
    }
}