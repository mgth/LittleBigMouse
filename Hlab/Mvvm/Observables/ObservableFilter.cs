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
using System.Collections;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Collections.Specialized;
using System.ComponentModel;
using System.Diagnostics;
using System.Linq;
using System.Threading;
using Hlab.Mvvm.Commands;
using Hlab.Notify;

namespace Hlab.Mvvm.Observables
{


    public class ObservableFilter<T> : ReadOnlyObservableCollection<T>, ITriggable, ILockable, INotifyCollectionChanged
        where T : INotifyPropertyChanged
    {
        private class Filter
        {
            public string Name { get; set; }
            public Func<T, bool> Expression { get; set; } = null;
            //public Func<IEnumerable<T>, IEnumerable<T>> Func { get; set; }
            public int Order { get; set; }
        }
        public ReaderWriterLockSlim Lock { get; } = new ReaderWriterLockSlim();

        private readonly ReaderWriterLockSlim _lockFilters = new ReaderWriterLockSlim();
        private readonly List<Filter> _filters = new List<Filter>();

        private readonly ObservableCollection<T> _collection;

        public ObservableFilter() : this(new ObservableCollection<T>())
        {
        }

        protected ObservableFilter(ObservableCollection<T> col) : base(col)
        {
            _collection = col;
        }

        public ObservableFilter<T> AddFilter(Func<T, bool> expr, int order = 0, string name = null)
        {
            _lockFilters.EnterWriteLock();
            try
            {
                if (name != null) RemoveFilter(name);
                _filters.Add(new Filter
                {
                    Name = name,
                    Expression = expr,
                    Order = order,
                });
                return this;                
            }
            finally { _lockFilters.ExitWriteLock(); }
        }
        public ObservableFilter<T> RemoveFilter(string name)
        {
            _lockFilters.EnterWriteLock();
            try
            {
                foreach (Filter f in _filters.Where(f => f.Name == name).ToList())
                {
                    _filters.Remove(f);
                }
                return this;
            }
            finally { _lockFilters.ExitWriteLock(); }
        }


        public class CreateHelper : IDisposable
        {
            public ObservableFilter<T> List = null;
            public T ViewModel = default(T);

            public TVm GetViewModel<TVm>() where TVm:class => ViewModel as TVm;

            public bool Done = true;

            public void Dispose()
            {
            }
        }

        private INotifyCollectionChanged _list = null;
        public ObservableFilter<T> Link(INotifyCollectionChanged list)
        {
            if(_list!=null) _list.CollectionChanged -= _list_CollectionChanged;
            Debug.Assert(Count==0);

            _list = list;
            if (_list == null) return this;

            if(((IList)_list).Count>0)
            _list_CollectionChanged(null,
                new NotifyCollectionChangedEventArgs(NotifyCollectionChangedAction.Add, _list as IList, 0));

            _list.CollectionChanged += _list_CollectionChanged; ;
            return this;
        }

        //public ObservableFilter<T> SetObserver(NotifyCollectionChangedEventHandler handler)
        //{
        //    _lockFilters.EnterReadLock();
        //    try
        //    {
        //        handler?.Invoke(null, new NotifyCollectionChangedEventArgs(NotifyCollectionChangedAction.Add, this.ToList()));
        //        CollectionChanged  += handler;
        //        return this;
        //    }
        //    finally { _lockFilters.ExitReadLock(); }
        //}
        public new event NotifyCollectionChangedEventHandler CollectionChanged
        {
            add => base.CollectionChanged += value;
            remove => base.CollectionChanged -= value;
        }

        private void _list_CollectionChanged(object sender, NotifyCollectionChangedEventArgs e)
        {
            switch (e.Action)
            {
                case NotifyCollectionChangedAction.Add:
                    Debug.Assert(e.NewItems != null);
                    //Debug.Assert(e.NewItems.Count == (_list as IList)?.Count - Count);
                    {
                        foreach (var item in e.NewItems.OfType<T>())
                        {
                            if (Match(item))
                            {
                                if(_collection.Contains(item))
                                { }
                                else
                                _collection.Add(item);
                            }
                        }
                    }
                     break;
                case NotifyCollectionChangedAction.Remove:

                    Debug.Assert(e.OldItems!=null);
                    {
                        foreach (var item in e.OldItems.OfType<T>())
                        {
                            _lockFilters.EnterWriteLock();
                            try
                            {

                                if (Contains(item)) _collection.Remove(item);
                            } finally { _lockFilters.ExitWriteLock(); }
                        }                    
                    }
                    
                    break;
                case NotifyCollectionChangedAction.Replace:
                    throw new NotImplementedException("Replace not implemented");
                case NotifyCollectionChangedAction.Move:
                    throw new NotImplementedException("Move not implemented");
                case NotifyCollectionChangedAction.Reset:
 //                       base.Clear();
                    ;
                    break;
                default:
                    throw new ArgumentOutOfRangeException();
            }

            //Debug.Assert(Count == (_list as IList)?.Count);


        }

        private bool Match(T item)
        {
            _lockFilters.EnterReadLock();
            try
            {
                if (_filters == null) return true;
                return item != null && _filters.Where(filter => filter.Expression != null).All(filter => filter.Expression(item));
            }
            finally
            {
                _lockFilters.ExitReadLock();
            }
        }

        public void OnTrigged()
        {
            if(_list is IEnumerable<T> list)
            foreach (var item in list)
            {
                if (_collection.Contains(item))
                {
                    if (!Match(item)) _collection.Remove(item);
                }
                else
                {
                    if(Match(item)) _collection.Add(item);
                }
            }
        }
    }
}
