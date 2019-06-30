using System;
using HLab.Notify.PropertyChanged;

namespace LittleBigMouse.ScreenConfigs
{
    public class ScreenRatioRatio : ScreenRatio<ScreenRatioRatio>
    {
        public ScreenRatioRatio(IScreenRatio ratioA, IScreenRatio ratioB)
        {
            SourceA = ratioA;
            SourceB = ratioB;
            Initialize();
        }

        public IScreenRatio SourceA { get; }
        public IScreenRatio SourceB { get; }

        public override double X
        {
            get => _x.Get();
            set => throw new NotImplementedException();
        }
        private readonly IProperty<double> _x = H.Property<double>(c => c
            .On(e => e.SourceA.X)
            .On(e => e.SourceB.X)
            .When(s => s.SourceA != null)
            .When(s => s.SourceB != null)
            .Set(s => s.SourceA.X * s.SourceB.X)
        );

        public override double Y
        {
            get => _y.Get();
            set => throw new NotImplementedException();
        }
        private readonly IProperty<double> _y = H.Property<double>(nameof(Y), c => c
            .On(e => e.SourceA.Y)
            .On(e => e.SourceB.Y)
            .When(s => s.SourceA != null)
            .When(s => s.SourceB != null)
            .Set(s => s.SourceA.Y * s.SourceB.Y));
    }
}