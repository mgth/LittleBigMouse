using System;
using System.Collections.ObjectModel;
using System.Windows;
using Hlab.Mvvm;
using Hlab.Notify;
using LbmScreenConfig;

namespace LittleBigMouse.Control.Core
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
