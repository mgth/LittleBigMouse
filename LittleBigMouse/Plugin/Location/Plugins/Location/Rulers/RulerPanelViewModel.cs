using System.Windows;
using Hlab.Mvvm;
using Hlab.Notify;
using LbmScreenConfig;

namespace LittleBigMouse.LocationPlugin.Plugins.Location.Rulers
{
    public class RulerPanelViewModel : ViewModel
    {
        public RulerPanelViewModel(Screen screen, Screen drawOn)
        {
            using (this.Suspend())
            {
                Screen = screen;
                DrawOn = drawOn;                
            }
        }
        public bool Enabled
        {
            get => this.Get<bool>(); set => this.Set(value);
        }
        public Screen DrawOn
        {
            get => this.Get<Screen>();
            set => this.Set(value);
        }
        public Screen Screen
        {
            get => this.Get<Screen>();
            set => this.Set(value);
        }
        public Visibility Visibility
        {
            get => this.Get<Visibility>(); set => this.Set(value);
        }

        public RulerViewModel TopRuler => this.Get(()=>new RulerViewModel(Screen,DrawOn,RulerViewModel.RulerSide.Top));
        public RulerViewModel RightRuler => this.Get(()=>new RulerViewModel(Screen,DrawOn,RulerViewModel.RulerSide.Right));
        public RulerViewModel BottomRuler => this.Get(()=>new RulerViewModel(Screen,DrawOn,RulerViewModel.RulerSide.Bottom));
        public RulerViewModel LeftRuler => this.Get(()=>new RulerViewModel(Screen,DrawOn,RulerViewModel.RulerSide.Left));

        [TriggedOn(nameof(DrawOn), "MmToDipRatio", "X")]
        public double RulerWidth => this.Get(() //=> 0);
            => 30 * DrawOn.MmToDipRatio.X);

        [TriggedOn(nameof(DrawOn), "MmToDipRatio", "Y")]
        public double RulerHeight => this.Get(()
            => 30 * DrawOn.MmToDipRatio.Y);
    }
}
