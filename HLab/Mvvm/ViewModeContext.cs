/*
  HLab.Mvvm
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

  This file is part of HLab.Mvvm.

    HLab.Mvvm is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    HLab.Mvvm is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/
using System;
using System.Collections.Concurrent;
using System.ComponentModel;
using System.Windows;
using HLab.Notify;

namespace HLab.Mvvm
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
