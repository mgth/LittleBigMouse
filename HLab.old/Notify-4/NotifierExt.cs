/*
  HLab.Notify.4
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

  This file is part of HLab.Notify.4.

    HLab.Notify.4 is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    HLab.Notify.4 is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/
using System;
using System.Collections;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Collections.Specialized;
using System.ComponentModel;
using System.Diagnostics;
using System.Linq;
using System.Reflection;
using System.Runtime.CompilerServices;
using HLab.Base;

namespace HLab.Notify
{
    public static class NotifierExt
    {
        public static Notifier GetNotifier(this /*INotifyPropertyChanged*/ object obj)
            => NotifierService.D.GetNotifier(obj);

        public static SuspenderToken Suspend(this INotifyPropertyChanged n)
            => n.GetNotifier().Suspend.Get();
        public static void Add(this INotifyPropertyChanged n, PropertyChangedEventHandler handler)
            => n.GetNotifier().Add(n,handler);

        public static void Remove(this INotifyPropertyChanged n, PropertyChangedEventHandler handler)
            => n.GetNotifier().Remove(n,handler);

        public static T Get<T>(this INotifyPropertyChanged n,
            [CallerMemberName] string propertyName = null)
            => n.GetNotifier().Get<T>(n, old => default(T), propertyName);

        public static T Get<T>(this INotifyPropertyChanged n, Func<T> getter,
            [CallerMemberName] string propertyName = null)
            => n.GetNotifier().Get<T>(n, old => getter(), propertyName);

        public static T Get<T>(this INotifyPropertyChanged n, Func<T, T> getter,
            [CallerMemberName] string propertyName = null)
            => n.GetNotifier().Get<T>(n, getter, propertyName);

        public static bool Set<T>(this INotifyPropertyChanged n,
            T value,
            Action<T, T> postUpdateAction,
            [CallerMemberName] string propertyName = null)
            => n.GetNotifier().Set(n, value, propertyName, postUpdateAction);

        public static bool Set<T>(this INotifyPropertyChanged n,
            T value,
            [CallerMemberName] string propertyName = null)
            => n.GetNotifier().Set(n, value, propertyName, null);

        public static void Subscribe(this INotifyPropertyChanged n)
        {
            var notifier = n.GetNotifier();

            foreach (var method in n.GetType().GetMethods())
            {
                foreach (var triggedOn in method.GetCustomAttributes().OfType<TriggedOn>())
                {
                    switch (method.GetParameters().Length)
                    {
                        case 0:
                            n.Subscribe(args => method.Invoke(n, null), triggedOn.Pathes);
                            break;
                        case 1:
                            n.Subscribe(args => method.Invoke(n, new object[]{ args }), triggedOn.Pathes);
                            break;
                    }                   
                }
            }

            foreach (var property in n.GetType().GetProperties())
            {
                var name = property.Name;
                foreach (var triggedOn in property.GetCustomAttributes().OfType<TriggedOn>())
                {
                    if (typeof(ITriggable).IsAssignableFrom(property.PropertyType))
                    {
                        n.Subscribe(a =>
                        {
                            if (notifier.IsSet(property))
                                notifier.Get<ITriggable>(n,null, name).OnTrigged();
                            else
                                notifier.OnPropertyChanged(name);
                            //property.GetMethod.Invoke(n, null);

                        }, triggedOn.Pathes);                        
                    }
                    else
                    {
                        n.Subscribe(a =>
                        {

                            if (notifier.IsSet(property))
                                notifier.Update(property);
                            else
                                notifier.OnPropertyChanged(name);
                            //property.GetMethod.Invoke(n, null);

                        }, triggedOn.Pathes);
                    }
                }
            }
        }

        public static void UnSubscribe(this INotifyPropertyChanged n, Action<NotifierPropertyChangedEventArgs> action,
            IList<string> targets)
        {
            if (PropertiesHandlers.TryRemove(Tuple.Create(action, targets), out PropertyChangedEventHandler h))
            {
                if (targets.Count > 1)
                {
                    if (n.GetType().GetProperty(targets[0])?.GetValue(n) is INotifyPropertyChanged value)
                    {
                        value.UnSubscribe(action, targets.Skip(1).ToArray());
                    }
                }
                n.PropertyChanged -= h;
            }
        }
        public static void UnSubscribe(this INotifyCollectionChanged n, Action<NotifierPropertyChangedEventArgs> action,
            IList<string> targets)
        {
            if (CollectionHandlers.TryRemove(Tuple.Create(action, targets), out NotifyCollectionChangedEventHandler h))
            {
                if (targets.Count > 1)
                {
                    if (n is IList oldItems)
                        foreach (var item in oldItems)
                        {
                            if (targets.Count > 1)
                            {
                                var newTargets = targets.Skip(1).ToArray();

                                if (item is INotifyCollectionChanged oldCollection)
                                    oldCollection.UnSubscribe(action, newTargets);

                                if (item is INotifyPropertyChanged oldValue)
                                    oldValue.UnSubscribe(action, newTargets);

                            }

                            action(new NotifierPropertyChangedEventArgs("Item", item, null));
                        }
                }
                n.CollectionChanged -= h;
            }
        }

        private static readonly
            ConcurrentDictionary<Tuple<Action<NotifierPropertyChangedEventArgs>, IList<string>>, PropertyChangedEventHandler>
            PropertiesHandlers =
                new ConcurrentDictionary<Tuple<Action<NotifierPropertyChangedEventArgs>, IList<string>>,
                    PropertyChangedEventHandler>();

        private static readonly
            ConcurrentDictionary<Tuple<Action<NotifierPropertyChangedEventArgs>, IList<string>>, NotifyCollectionChangedEventHandler>
            CollectionHandlers =
                new ConcurrentDictionary<Tuple<Action<NotifierPropertyChangedEventArgs>, IList<string>>,
                    NotifyCollectionChangedEventHandler>();

        public static void Subscribe(
            this INotifyPropertyChanged n, 
            Action<NotifierPropertyChangedEventArgs> action,
            IList<string> targets)
        {
            Debug.Assert(targets.Count > 0);

            {
                void H(object sender, PropertyChangedEventArgs args)
                {
                    if (args.PropertyName == targets[0] && args is NotifierPropertyChangedEventArgs argt)
                    {
                        action(argt);
                    }
                }


                n.PropertyChanged += H;
                PropertiesHandlers.TryAdd(Tuple.Create(action, targets), H);
            }

            if (targets.Count > 1)
            {
                void H(object sender, PropertyChangedEventArgs args)
                {
                    if (args.PropertyName == targets[0] && args is NotifierPropertyChangedEventArgs argt)
                    {
                        var newTargets = targets.Skip(1).ToArray();

                        if (argt.OldValue is INotifyCollectionChanged oldCollection && newTargets[0] == "Item")
                            oldCollection.UnSubscribe(action, newTargets);

                        if (argt.OldValue is INotifyPropertyChanged oldValue)
                            oldValue.UnSubscribe(action, newTargets);

                        if (argt.NewValue is INotifyPropertyChanged newValue)
                            newValue.Subscribe(action, newTargets);

                        if (argt.NewValue is INotifyCollectionChanged newCollection && newTargets[0]=="Item")
                            newCollection.Subscribe(action, newTargets);

                        action(argt);
                    }
                }


                var property = n.GetType().GetProperty(targets[0]);
                if(property!=null && !(n is INotifyCollectionChanged && targets[0]=="Item"))
                    H(null, new NotifierPropertyChangedEventArgs(targets[0], null, property.GetValue(n)));

                n.PropertyChanged += H;
                PropertiesHandlers.TryAdd(Tuple.Create(action, targets), H);
            }

        }

        public static void Subscribe(
            this INotifyCollectionChanged n, 
            Action<NotifierPropertyChangedEventArgs> action,
            IList<string> targets)
        {
            Debug.Assert(targets[0]=="Item");


            void H(object sender, NotifyCollectionChangedEventArgs args)
            {
                //if (args.PropertyName == targets[0] && args is NotifierPropertyChangedEventArgs argt)
                //{
                //    action(argt);
                //}
                var newTargets = targets.Skip(1).ToArray();

                if (args.NewItems!=null)
                    foreach (var item in args.NewItems)
                    {
                        if (targets.Count > 1)
                        {

                            if (item is INotifyPropertyChanged newValue)
                                newValue.Subscribe(action, newTargets);

                            if (item is INotifyCollectionChanged newCollection)
                                newCollection.Subscribe(action, newTargets);

                        }

                        action(new NotifierPropertyChangedEventArgs("Item",null,item));
                    }

                if(args.OldItems!=null)
                    foreach (var item in args.OldItems)
                    {
                        if (targets.Count > 1)
                        {
                            if (item is INotifyCollectionChanged oldCollection)
                                oldCollection.UnSubscribe(action, newTargets);

                            if (item is INotifyPropertyChanged oldValue)
                                oldValue.UnSubscribe(action, newTargets);

                        }

                        action(new NotifierPropertyChangedEventArgs("Item",item,null));
                    }
            }

            if(n is IList newItems)
            {
                H(null,
                    new NotifyCollectionChangedEventArgs(NotifyCollectionChangedAction.Add,newItems,0));

            }

            CollectionHandlers.TryAdd(Tuple.Create(action, targets), H);
            n.CollectionChanged += H;
        }

    }
}