using System;
using System.Collections.Generic;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using Erp.Base.Commands;
using Erp.Notify;
using Erp.Mvvm;

namespace LittleBigMouse_Control.Rulers
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
