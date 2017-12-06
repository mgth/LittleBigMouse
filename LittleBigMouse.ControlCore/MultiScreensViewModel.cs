using System;
using System.Collections.ObjectModel;
using System.Collections.Specialized;
using System.Linq;
using System.Windows;
using System.Windows.Controls;
using Erp.Notify;
using LbmScreenConfig;
using Erp.Mvvm;

namespace LittleBigMouse.ControlCore
{
    public class MultiScreensViewModel : ViewModel, IPresenterViewModel
    {
        public MultiScreensViewModel()
        {
            this.Subscribe();
        }

        public MainViewModel MainViewModel
        {
            get => this.Get<MainViewModel>();
            set => this.Set(value);
        }

        public Type ViewMode
        {
            get => this.Get(() => typeof(ViewModeDefault/*ViewModeScreenLocation*/));
            set => this.Set(value);
        }

        public ViewModeContext Context => this.Get(
            () => new ViewModeContext(nameof(MultiScreensViewModel))
            .AddCreator<ScreenFrameViewModel>(vm => vm.Presenter = this ));

        public Size Size { get => this.Get<Size>(); set => this.Set(value); }


        public ScreenConfig Config
        {
            get => this.Get<ScreenConfig>();
            set => this.Set(value);
        }


        public ObservableCollection<ScreenFrameViewModel> ScreenFrames = new ObservableCollection<ScreenFrameViewModel>();


    }
}
