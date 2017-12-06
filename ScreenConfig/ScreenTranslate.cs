using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Windows;
using Erp.Notify;

namespace LbmScreenConfig
{
    public static class ScreenTranslateExt
    {
        public static ScreenSize Translate(this ScreenSize source, Vector translation) => new ScreenTranslate(source, translation);
    }

    public class ScreenTranslate : ScreenSize
    {
        public ScreenTranslate(ScreenSize source, Vector? translation = null)
        {
            Source = source;
            Translation = translation ?? new Vector();
            this.Subscribe();
        }

        public Vector Translation
        {
            get => this.Get<Vector>();
            set => this.Set(value);
        }

        [TriggedOn(nameof(Source), "Width")]
        public override double Width
        {
            get => this.Get(() => Source.Width);
            set => this.Set(Source.Width = value);
        }

        [TriggedOn(nameof(Source), "Height")]
        public override double Height
        {
            get => this.Get(() => Source.Height);
            set => this.Set(Source.Height = value);
        }

        [TriggedOn(nameof(Source), "X")]
        [TriggedOn(nameof(Translation))]
        public override double X
        {
            get => this.Get(() => Source.X + Translation.X);
            set => Source.X = value - Translation.X;
        }

        [TriggedOn(nameof(Source), "Y")]
        [TriggedOn(nameof(Translation))]
        public override double Y
        {
            get => this.Get(() => Source.Y + Translation.Y);
            set => Source.Y = value - Translation.Y;
        }

        [TriggedOn(nameof(Source), "TopBorder")]
        public override double TopBorder
        {
            get => this.Get(() => Source.TopBorder);
            set => Source.TopBorder = value;
        }

        [TriggedOn(nameof(Source), "RightBorder")]
        public override double RightBorder
        {
            get => this.Get(() => Source.RightBorder);
            set => Source.RightBorder = value;
        }

        [TriggedOn(nameof(Source), "BottomBorder")]
        public override double BottomBorder
        {
            get => this.Get(() => Source.BottomBorder);
            set => Source.BottomBorder = value;
        }

        [TriggedOn(nameof(Source), "LeftBorder")]
        public override double LeftBorder
        {
            get => this.Get(() => Source.LeftBorder);
            set => Source.LeftBorder = value;
        }
    }
}
