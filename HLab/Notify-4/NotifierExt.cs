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

        public static void SubscribeNotifier(this INotifyPropertyChanged n) => n.GetNotifier().Subscribe(n);
        

        public static void UnSubscribeNotifier(this INotifyPropertyChanged n, PropertyChangedEventHandler action,
            IList<string> targets)
        {
            if (PropertiesHandlers.TryRemove(Tuple.Create(action, targets), out PropertyChangedEventHandler h))
            {
                if (targets.Count > 1)
                {
                    if (n.GetType().GetProperty(targets[0])?.GetValue(n) is INotifyPropertyChanged value)
                    {
                        value.UnSubscribeNotifier(action, targets.Skip(1).ToArray());
                    }
                }
                n.PropertyChanged -= h;
            }
        }
        public static void UnSubscribeNotifier(this INotifyCollectionChanged n, PropertyChangedEventHandler action,
            IList<string> targets)
        {
            if (CollectionHandlers.TryRemove(Tuple.Create(action, targets), out NotifyCollectionChangedEventHandler h))
            {
                if (targets.Count > 1)
                {
                    var newTargets = targets.Skip(1).ToArray();

                    if (n is IList oldItems)
                        foreach (var item in oldItems)
                        {
                            UnSubscribeNotifier(item,action,newTargets);
                            action(n, new NotifierPropertyChangedEventArgs("Item", item, null));
                        }
                }
                n.CollectionChanged -= h;
            }
        }

        public static void UnSubscribeNotifier(object n, PropertyChangedEventHandler action,
            IList<string> targets)
        {
                if (n is INotifyCollectionChanged oldCollection)
                    oldCollection.UnSubscribeNotifier(action, targets);

                if (n is INotifyPropertyChanged oldValue)
                    oldValue.UnSubscribeNotifier(action, targets);
        }

        private static readonly
            ConcurrentDictionary<Tuple<PropertyChangedEventHandler, IList<string>>, PropertyChangedEventHandler>
            PropertiesHandlers =
                new ConcurrentDictionary<Tuple<PropertyChangedEventHandler, IList<string>>,
                    PropertyChangedEventHandler>();

        private static readonly
            ConcurrentDictionary<Tuple<PropertyChangedEventHandler, IList<string>>, NotifyCollectionChangedEventHandler>
            CollectionHandlers =
                new ConcurrentDictionary<Tuple<PropertyChangedEventHandler, IList<string>>,
                    NotifyCollectionChangedEventHandler>();

        private static void Unsubscribe(object value, IList<string> targets, PropertyChangedEventHandler handler)
        {
            if (value is INotifyCollectionChanged oldCollection && targets[0] == "Item")
            {
                oldCollection.UnSubscribeNotifier(handler, targets);
                return;
            }

            if (value is INotifyPropertyChanged oldNotifier)
                oldNotifier.UnSubscribeNotifier(handler, targets);
        }
        private static void Subscribe(object value, IList<string> targets, PropertyChangedEventHandler handler)
        {
            if (value is INotifyCollectionChanged newCollection && targets[0] == "Item")
            {
                newCollection.SubscribeNotifier(handler, targets);
                return;
            }

            if (value is INotifyPropertyChanged newNotifier)
                newNotifier.SubscribeNotifier(handler, targets);
        }


        public static void SubscribeNotifier(
            this INotifyPropertyChanged n, 
            PropertyChangedEventHandler handler,
            IList<string> targets)
        {
            Debug.Assert(targets.Count > 0);
            var target = targets[0];

            var broker = n.GetBroker();

            if (targets.Count > 1)
            {
                var newTargets = targets.Skip(1).ToArray();
                var newTarget = newTargets[0];
                void H(object sender, PropertyChangedEventArgs args)
                {
                    if (args is NotifierPropertyChangedEventArgs argt)
                    {
                        Unsubscribe(argt.OldValue,newTargets,handler);
                        Subscribe(argt.NewValue,newTargets,handler);
                    }
                    //handler(sender, args);
                }

                var value = broker.Subscribe(target,H);

                if(value!=null)
                    Subscribe(value,newTargets,handler);

                PropertiesHandlers.TryAdd(Tuple.Create(handler, targets), H);
            }

            //if (n is INotifyCollectionChanged && target == "Item") return;
            broker.Subscribe(target, handler);
            PropertiesHandlers.TryAdd(Tuple.Create(handler, targets), handler);
        }

        public static void SubscribeNotifier(
            this INotifyCollectionChanged n,
            PropertyChangedEventHandler action,
            IList<string> targets)
        {
            Debug.Assert(targets[0]=="Item");

            var newTargets = targets.Skip(1).ToArray();

            void H(object sender, NotifyCollectionChangedEventArgs args)
            {
                //if (args.PropertyName == targets[0] && args is NotifierPropertyChangedEventArgs argt)
                //{
                //    action(argt);
                //}

                if (args.NewItems!=null)
                    foreach (var item in args.NewItems)
                    {
                        if (targets.Count > 1)
                        {

                            if (item is INotifyPropertyChanged newValue)
                                newValue.SubscribeNotifier(action, newTargets);

                            if (item is INotifyCollectionChanged newCollection)
                                newCollection.SubscribeNotifier(action, newTargets);

                        }

                        action(sender, new NotifierPropertyChangedEventArgs("Item",null,item));
                    }

                if(args.OldItems!=null)
                    foreach (var item in args.OldItems)
                    {
                        if (targets.Count > 1)
                        {
                            if (item is INotifyCollectionChanged oldCollection)
                                oldCollection.UnSubscribeNotifier(action, newTargets);

                            if (item is INotifyPropertyChanged oldValue)
                                oldValue.UnSubscribeNotifier(action, newTargets);

                        }

                        action(sender, new NotifierPropertyChangedEventArgs("Item",item,null));
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

        //public static void SubscribeOneToMany<T>(
        //    this object target, 
        //    IList<T> list,
        //    string property)
        //{
        //    NotifierService.D.GetNotifierClass(typeof(T)).GetProperty(property).RegisterOneToMany(target,(IList)list);
        //}
    }
    }