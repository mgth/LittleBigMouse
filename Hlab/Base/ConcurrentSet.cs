using System;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Linq;
using System.Threading.Tasks;

namespace Hlab.Base
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
