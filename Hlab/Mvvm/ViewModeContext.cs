using System;
using System.Collections.Concurrent;
using System.ComponentModel;
using System.Windows;
using Hlab.Notify;

namespace Hlab.Mvvm
{
    public class ViewModeContext
    {
        private readonly ConcurrentDictionary<Type, ViewModelCache> _cache = new ConcurrentDictionary<Type, ViewModelCache>();
        private readonly ConcurrentDictionary<Type, ConcurrentQueue<Action<object>>> _creators;

        private ViewModelCache GetCache(Type viewMode) => _cache.GetOrAdd(viewMode, (vm) => 
        {
            var c = new ViewModelCache(_creators, vm);

            c.ViewDataContextChanged += Cache_ViewDataContextChanged;
            return c;
        });


        public string Name { get; }

        public ViewModeContext(string name)
        {
            Name = name;
            _creators = new ConcurrentDictionary<Type, ConcurrentQueue<Action<object>>>();

            AddCreator<INotifyPropertyChanged>(viewModel => viewModel.Set(this, "ActualViewModeContext"));
        }

        public ViewModeContext AddCreator<T>(Action<T> action)
        {
            var list = _creators.GetOrAdd(typeof(T), t => new ConcurrentQueue<Action<object>>());
            list.Enqueue(e => action((T) e));
            return this;
        }


        public object GetLinked(object o, Type viewMode, Type viewClass) => GetCache(viewMode).GetLinked(o, viewClass);

        public object GetLinked<T>(object o, Type viewClass) => GetLinked(o, typeof(T), viewClass);

        public object GetLinked<T>(object o) => GetLinked(o, typeof(T), typeof(IViewClassDefault));

        public FrameworkElement GetView(object baseObject, Type viewMode, Type viewClass)
            => GetCache(viewMode).GetView(baseObject,viewClass);
        public FrameworkElement GetView(object baseObject, Type viewMode)
            => GetCache(viewMode).GetView(baseObject, typeof(IViewClassDefault));
        public FrameworkElement GetView<T>(object baseObject, Type viewClass)
            => GetCache(typeof(T)).GetView(baseObject, viewClass);
        public FrameworkElement GetView<T>(object baseObject)
            => GetCache(typeof(T)).GetView(baseObject, typeof(IViewClassDefault));


        public static event DependencyPropertyChangedEventHandler ViewDataContextChanged;
        private void Cache_ViewDataContextChanged(object sender, DependencyPropertyChangedEventArgs e)
        {
            ViewDataContextChanged?.Invoke(sender, e);
        }
    }
}
