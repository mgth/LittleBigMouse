using System;
using System.Collections.ObjectModel;
using System.Collections.Specialized;
using System.Linq;
using System.Windows;
using System.Windows.Controls;
using LbmScreenConfig;
using NotifyChange;

namespace LittleBigMouse_Control
{
    internal class MultiScreensViewModel : PresenterViewModel
    {
        public override Type ViewType => typeof(MultiScreensView);
        public MultiScreensViewModel()
        {
            ScreensCanvas.SizeChanged += (sender, args) => RaiseProperty("Size");
            ScreenFrames.CollectionChanged += ScreenFrames_CollectionChanged;
        }

        private void ScreenFrames_CollectionChanged(object sender, NotifyCollectionChangedEventArgs e)
        {
            if (e.OldItems != null)
                foreach (ScreenFrameViewModel frame in e.OldItems)
                {
                    ScreensCanvas.Children.Remove(frame.View);
                }

            if (e.NewItems != null)
                foreach (ScreenFrameViewModel frame in e.NewItems)
                {
                    ScreensCanvas.Children.Add(frame.View);
                }
        }

        public ScreenConfig Config
        {
            get { return GetProperty<ScreenConfig>(); }
            set { SetAndWatch(value); }
        }

        public Canvas ScreensCanvas { get; } = new Canvas();



        public ObservableCollection<ScreenFrameViewModel> ScreenFrames = new ObservableCollection<ScreenFrameViewModel>();

        [DependsOn("Config.Selected")]
        private void BringSelectedToFront()
        {
            ScreenFrameViewModel frame = ScreenFrames.FirstOrDefault(vm => vm.Screen.Selected);
            if (frame != null)
            {
                ScreensCanvas.Children.Remove(frame.View);
                ScreensCanvas.Children.Add(frame.View);               
            }
        }


        [DependsOn(nameof(Config))]
        private void UpdateConfig()
        {
            if (Config == null) return;
            AllScreens_CollectionChanged(Config.AllScreens, new NotifyCollectionChangedEventArgs(NotifyCollectionChangedAction.Add,Config.AllScreens));
            Config.AllScreens.CollectionChanged += AllScreens_CollectionChanged;
        }

        private void AllScreens_CollectionChanged(object sender, NotifyCollectionChangedEventArgs e)
        {
            if(e.NewItems!=null)
                foreach (Screen screen in e.NewItems)
                {
                    ScreenFrameViewModel vm = ScreenFrames.FirstOrDefault(v => v.Screen.Equals(screen) );
                    if (vm == null)
                        ScreenFrames.Add(new ScreenFrameViewModel
                        {
                            Screen = screen,
                            Presenter = this
                        });
                }
            if (e.OldItems != null)
                foreach (Screen screen in e.OldItems)
                {
                    ScreenFrameViewModel vm = ScreenFrames.FirstOrDefault(v => v.Screen.Equals(screen));
                    if (vm!=null) ScreenFrames.Remove(vm);
                }
        }

        [DependsOn("Size", "Config.MovingPhysicalOutsideBounds")]
        private void UpdateRatio()
        {
            if (Config == null) return;

            Rect all = Config.MovingPhysicalOutsideBounds;

            double ratio = 0;

            if (all.Width * all.Height > 0)
            {
                ratio = Math.Min(
                    ScreensCanvas.ActualWidth / all.Width,
                    ScreensCanvas.ActualHeight / all.Height
                    );
            }
            Ratio = ratio;
        }
        public override double PhysicalToUiX(double x) 
            => (x - Config.MovingPhysicalOutsideBounds.Left) * Ratio 
            + (ScreensCanvas.ActualWidth 
            - Config.MovingPhysicalOutsideBounds.Width * Ratio) / 2;
        public override double PhysicalToUiY(double y) 
            => (y - Config.MovingPhysicalOutsideBounds.Top) * Ratio 
            + (ScreensCanvas.ActualHeight 
            - Config.MovingPhysicalOutsideBounds.Height * Ratio) / 2;

        public Point PhysicalToUi(Point p)
        {
            return new Point(
                PhysicalToUiX(p.X),
                PhysicalToUiY(p.Y)
                );
        }
        public Point UiToPhysical(Point p)
        {
            Rect all = Config.MovingPhysicalOutsideBounds;

            return new Point(
                (p.X / Ratio) + all.Left,
                (p.Y / Ratio) + all.Top
                );
        }
        public Vector UiToPhysical(Vector V)
        {
            return new Vector(
                (V.X / Ratio),
                (V.Y / Ratio)
                );
        }
    }
}
