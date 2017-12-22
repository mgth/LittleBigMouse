/*
  HLab.Base
  Copyright (c) 2017 Mathieu GRENET.  All right reserved.

  This file is part of HLab.Base.

    HLab.Base is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    HLab.Base is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with MouseControl.  If not, see <http://www.gnu.org/licenses/>.

	  mailto:mathieu@mgth.fr
	  http://www.mgth.fr
*/
using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Linq;
using System.Threading.Tasks;

namespace HLab.Base
{
    public class ConcurrentSet<T>
    {
        private readonly object _lock = new object();
        private readonly ObservableCollection<T> _set = new ObservableCollection<T>();

        public bool Add(T item)
        {
            lock (_lock)
            {
                if (_set.Contains(item)) return false;
                _set.Add(item);
                return true;
            }
        }
        public bool Remove(T item)
        {
            lock (_lock)
            {
                return _set.Remove(item);
            }
        }

        public void ParalleleForEach(Action<T> action)
        {
            List<T> list;
            var @lock = new object();
            var removed = new List<T>();
            var added = new List<T>();
            lock (_lock)
            {
                _set.CollectionChanged += (sender, a) =>
                {
                    lock (@lock)
                    {
                        if (a.NewItems!=null) added.AddRange(a.NewItems.OfType<T>());
                        if (a.OldItems != null) removed.AddRange(a.OldItems.OfType<T>());
                    }                    
                };
                list = _set.ToList();                
            }

            while (list.Count > 0)
            {
                Parallel.ForEach(list, obj =>
                {
                    lock (@lock)
                    {
                        if (removed.Contains(obj))
                        {
                            removed.Remove(obj);
                            return;
                        }
                    }
                    action(obj);
                });

                lock(@lock)
                {
                    list = added.ToList();
                    added.Clear();
                }
            }
        }
        public void ForEach(Action<T> action)
        {
            List<T> list;
            var @lock = new object();
            var removed = new List<T>();
            var added = new List<T>();
            lock (_lock)
            {
                _set.CollectionChanged += (sender, a) =>
                {
                    lock (@lock)
                    {
                        if (a.NewItems != null) added.AddRange(a.NewItems.OfType<T>());
                        if (a.OldItems != null) removed.AddRange(a.OldItems.OfType<T>());
                    }
                };
                list = _set.ToList();
            }

            while (list.Count > 0)
            {
                foreach(var obj in list)
                {
                    lock (@lock)
                    {
                        if (removed.Contains(obj))
                        {
                            removed.Remove(obj);
                            return;
                        }
                    }
                    action(obj);
                }

                lock (@lock)
                {
                    list = added.ToList();
                    added.Clear();
                }
            }
        }
        public List<T> ToList()
        {
            lock (_lock)
            {
                return _set.ToList();
            }
        }
    }
}
