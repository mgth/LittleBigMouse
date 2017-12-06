using System.Windows;
using Erp.Notify;

namespace LbmScreenConfig
{
    public static class ScreenRotateExt
    {
        public static ScreenRotate Rotate(this ScreenSize source, int rotation) => new ScreenRotate(source,rotation);
    }


    public class ScreenRotate : ScreenSize
    {
        public ScreenRotate(ScreenSize source, int rotation = 0)
        {
            this.Subscribe();
            Source = source;
            _rotation = rotation;

        }


        private readonly int _rotation;

        public int Rotation => this.Get(() => _rotation);
        //{
        //    get => this.Get<int>(()=>0);
        //    set => this.Set(value);
        //}
        public Vector Translation
        {
            get => this.Get<Vector>();
            set => this.Set(value);
        }

        [TriggedOn(nameof(Source), "Width")]
        [TriggedOn(nameof(Source), "Height")]
        [TriggedOn(nameof(Rotation))]
        public override double Width
        {
            get => this.Get(() => (Rotation % 2 == 0 ? Source.Width : Source.Height));
            set
            {
                switch (Rotation % 2)
                {
                    case 0:
                        Source.Width = value;
                        break;
                    case 1:
                        Source.Height = value;
                        break;
                }
            }
        }


        [TriggedOn(nameof(Source), "Width")]
        [TriggedOn(nameof(Source), "Height")]
        [TriggedOn(nameof(Rotation))]
        public override double Height
        {
            get => this.Get(() => Rotation % 2 == 1 ? Source.Width : Source.Height);
            set
            {
                switch (Rotation % 2)
                {
                    case 0: Source.Height = value;
                        break;
                    case 1: Source.Width = value;
                        break;
                }
            }
        }

        [TriggedOn(nameof(Source),"X")]
        public override double X
        {
            get => this.Get(() => Source.X);
            set => Source.X = value;
        }

        [TriggedOn(nameof(Source),"Y")]
        public override double Y
        {
            get => this.Get(() => Source.Y);
            set => Source.Y = value;
        }

        private double GetBorder(int border)
        {
            switch ((border+Rotation)%4)
            {
                case 0: return Source.TopBorder;
                case 1: return Source.RightBorder;
                case 2: return Source.BottomBorder;
                case 3: return Source.LeftBorder;
            }
            return -1;
            
        }
        private void SetBorder(int border, double value)
        {
            switch ((border + Rotation) % 4)
            {
                case 0:
                    Source.TopBorder = value;
                    break;
                case 1:
                    Source.RightBorder = value;
                    break;
                case 2:
                    Source.BottomBorder = value;
                    break;
                case 3:
                    Source.LeftBorder = value;
                    break;
            }
        }


        [TriggedOn(nameof(Source), "TopBorder")]
        [TriggedOn(nameof(Source), "RightBorder")]
        [TriggedOn(nameof(Source), "BottomBorder")]
        [TriggedOn(nameof(Source), "LeftBorder")]
        [TriggedOn(nameof(Rotation))]
        public override double TopBorder
        {
            get => this.Get(() => GetBorder(0));
            set => SetBorder(0,value);
        }

        [TriggedOn(nameof(Source), "TopBorder")]
        [TriggedOn(nameof(Source), "RightBorder")]
        [TriggedOn(nameof(Source), "BottomBorder")]
        [TriggedOn(nameof(Source), "LeftBorder")]
        [TriggedOn(nameof(Rotation))]
        public override double RightBorder
        {
            get => this.Get(() => GetBorder(1));
            set => SetBorder(1,value);
        }

        [TriggedOn(nameof(Source), "TopBorder")]
        [TriggedOn(nameof(Source), "RightBorder")]
        [TriggedOn(nameof(Source), "BottomBorder")]
        [TriggedOn(nameof(Source), "LeftBorder")]
        [TriggedOn(nameof(Rotation))]
        public override double BottomBorder
        {
            get => this.Get(() => GetBorder(2));
            set => SetBorder(2,value);
        }

        [TriggedOn(nameof(Source), "TopBorder")]
        [TriggedOn(nameof(Source), "RightBorder")]
        [TriggedOn(nameof(Source), "BottomBorder")]
        [TriggedOn(nameof(Source), "LeftBorder")]
        [TriggedOn(nameof(Rotation))]
        public override double LeftBorder
        {
            get => this.Get(() => GetBorder(3));
            set => SetBorder(3,value);
        }
    }
}