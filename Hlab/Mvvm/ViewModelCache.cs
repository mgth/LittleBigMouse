using System;
using System.Collections.Concurrent;
using System.ComponentModel;
using System.Linq;
using System.Runtime.CompilerServices;
using System.Windows;
using Hlab.Notify;

namespace Hlab.Mvvm
{
    public class ViewModelCache
    {
        private readonly ConditionalWeakTable<object, object> _linked = new ConditionalWeakTable<object, object>();
        private readonly ConcurrentDictionary<Type, ConcurrentQueue<Action<object>>> _creators;

        private readonly Type _viewMode;

        public ViewModelCache(ConcurrentDictionary<Type, ConcurrentQueue<Action<object>>> creators, Type viewMode)
        {
            _creators = creators;
            _viewMode = viewMode;
        }

        public object GetLinked(object baseObject,Type viewClass)
        {
            var linkedType = MvvmService.D.GetLinkedType(baseObject?.GetType(), _viewMode, viewClass);

            if (linkedType == null) return null;

            var created = false;
            object linked;

            if (baseObject != null && !typeof(FrameworkElement).IsAssignableFrom(linkedType))
            {

                linked = _linked.GetValue(baseObject.GetNotifier(), (o) =>
                {
                    created = true;
                    return Activator.CreateInstance(linkedType);
                });
            }
            else
            {
                created = true;
                linked = Activator.CreateInstance(linkedType);
            }

            if (linked != null && created)
            {
                switch (linked)
                {
                    case FrameworkElement fe:
                        fe.DataContext = baseObject;
                        Init(linked);
                        break;
                    case INotifyPropertyChanged vm:
                        using (vm.Suspend())
                        {
                            vm.SetModel(baseObject);
                            Init(linked);                            
                        }
                        break;
                }

                switch (baseObject)
                {
                    case INotifyPropertyChanged vm:
                        using (vm.Suspend())
                        {
                            vm.SetLinked(linked);
                        }
                        break;
                }

            }
            return linked;
        }

        public FrameworkElement GetView(object baseObject, Type viewClass)
        {

            if (viewClass == null) viewClass = typeof(IViewClassDefault);

            while (true)
            {
                var linked = GetLinked(baseObject,viewClass);

                if (linked == null)
                {

                    linked = GetLinked(baseObject, viewClass);

                    return new NotFoundView
                    {
                        Title = {Content = "View not found"},
                        Message = {Content = (baseObject?.GetType()?.ToString() ?? "??") + "\n" + _viewMode + "\n" + viewClass.FullName}
                    };
                }

                if (linked is FrameworkElement fe)
                    return fe;

                baseObject = linked;
            }
        }

        /// <summary>
        /// Initialise newly created linked using creators
        /// </summary>
        /// <param name="linked"></param>
        private void Init(object linked)
        {
            foreach (var type in _creators.Keys.Where(t => t.IsInstanceOfType(linked)))
            {
                foreach (var creator in _creators[type])
                {
                    creator(linked);
                }
            }
        }

        public event DependencyPropertyChangedEventHandler ViewDataContextChanged;
        private void View_DataContextChanged(object sender, DependencyPropertyChangedEventArgs e)
        {
            ViewDataContextChanged?.Invoke(sender, e);
        }
    }
}
