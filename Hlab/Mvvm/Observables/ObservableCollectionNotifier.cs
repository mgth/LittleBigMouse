using System;
using System.Collections;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Collections.Specialized;
using System.ComponentModel;
using System.Diagnostics;
using System.Linq;
using System.Reflection;
using System.Threading;
using System.Windows.Threading;
using Hlab.Notify;

namespace Hlab.Mvvm.Observables
{
    public static class ObservableCollectionExt
    {
        public static void Sort<T>(this ObservableCollectionNotifier<T> collection, Comparison<T> comparison)
            where T : INotifyPropertyChanged
        {
            var list = new List<T>(collection);
            list.Sort(comparison);

            for (int i = 0; i < list.Count; i++)
            {
                if (collection.IndexOf(list[i]) == i) continue;
                collection.Remove(list[i]);
                collection.Insert(i,list[i]);
            }
        }
    }

    public interface ILockable
    {
        ReaderWriterLockSlim Lock { get; }
    }

    public abstract class ObservableCollectionNotifier : INotifyPropertyChanged
    {
        protected Dispatcher Dispatcher;
        //public readonly object Lock = new object();

        public event PropertyChangedEventHandler PropertyChanged
        {
            add => this.Add(value);
            remove => this.Remove(value);
        }

        public abstract void SetObserver(NotifyCollectionChangedEventHandler handler);
    }

    public class ObservableCollectionNotifier<T> :
        ObservableCollectionNotifier,
        
        IList<T>, IList, IReadOnlyList<T>, INotifyCollectionChanged, ILockable
        where T : INotifyPropertyChanged
    {
        private readonly ObservableCollection<T> _list;
        public ReaderWriterLockSlim Lock { get; } = new ReaderWriterLockSlim();

        public event NotifyCollectionChangedEventHandler CollectionChanged;

        public ObservableCollectionNotifier()
        {
            Dispatcher = Dispatcher.CurrentDispatcher;

            //BindingOperations.EnableCollectionSynchronization(this, Lock);

            _list = new ObservableCollection<T>();
            _list.CollectionChanged += _list_CollectionChanged;
        }
        public virtual T Selected
        {
            get { lock (Lock) return this.Get(() => default(T)); }
            set { lock (Lock) this.Set(value); }
        }
        public override void SetObserver(NotifyCollectionChangedEventHandler handler)
        {
            Lock.EnterReadLock();
            try
            {
                handler?.Invoke(null, new NotifyCollectionChangedEventArgs(NotifyCollectionChangedAction.Add, this.ToList()));
                CollectionChanged += handler;
                //return this;
            }
            finally { Lock.ExitReadLock(); }
        }

        private readonly ConcurrentQueue<NotifyCollectionChangedEventArgs> _notifyQueue = new ConcurrentQueue<NotifyCollectionChangedEventArgs>();

        private void _list_CollectionChanged(object sender, NotifyCollectionChangedEventArgs e)
        {
                _notifyQueue.Enqueue(e);
        }

        private void OnCollectionChanged()
        {
            Count = _list.Count;

            while (_notifyQueue.TryDequeue(out var e))
            {

                var handlers = CollectionChanged?.GetInvocationList().OfType<NotifyCollectionChangedEventHandler>();
                if (handlers == null) return;

                //var toDispatch = new List<NotifyCollectionChangedEventHandler>();
                //var lockDispatch = new object();

                var arg = e;

                //Parallel.ForEach(handlers, (handler) =>
                foreach (var handler in handlers)
                {
                    if (handler.Target is DispatcherObject dispatcherObject && !dispatcherObject.CheckAccess())
                    {
//                        lock (lockDispatch) toDispatch.Add(handler);
                        dispatcherObject.Dispatcher.Invoke(DispatcherPriority.DataBind, handler, this, arg);
                    }
                    else
                        try
                        {
                            handler(handler.Target, arg);
                            //handler(this.GetProxy().Target, arg);
                            // note : this does not execute handler in target thread's context
                        }
                        catch(TargetInvocationException)
                        { }
                        catch (NullReferenceException)
                        { }
                
                } //);
                //foreach (var h in toDispatch) h(this.GetProxy().Target, arg);
            }

        }



        public IEnumerator<T> GetEnumerator() => _list.GetEnumerator();
        IEnumerator IEnumerable.GetEnumerator() => _list.GetEnumerator();

        public int Count
        {
            get => this.Get(() => _list.Count); private set => this.Set(value);
        }

        int IList.Add(object value)
        {
            Lock.EnterWriteLock();
            try
            {
                return ((IList)_list).Add(value);
            }
            finally { Lock.ExitWriteLock(); }
        }

        public virtual void Add(T item)
        {
            Lock.EnterWriteLock();
            try
            {
                _list.Add(item);
            }
            finally { Lock.ExitWriteLock(); }
            OnCollectionChanged();
        }
        public bool AddUnique(T item)
        {
            Lock.EnterWriteLock();
            try
            {
                if (Contains(item)) return false;
                _list.Add(item);
            }
            finally { Lock.ExitWriteLock(); }

            OnCollectionChanged();
            return true;
        }

        void IList.Remove(object value)
        {
            Lock.EnterWriteLock();
            try
            {
                ((IList)_list).Remove(value);
            }
            finally { Lock.ExitWriteLock(); }
            OnCollectionChanged();
        }

        public bool Remove(T item)
        {
            bool r = false;
            Lock.EnterWriteLock();
            try
            {
                r = _list.Remove(item);
            }
            finally { Lock.ExitWriteLock(); }
            OnCollectionChanged();
            return r;
        }

        public void RemoveAt(int index)
        {
            Lock.EnterWriteLock();
            try
            {
                _list.RemoveAt(index);
            }
            finally { Lock.ExitWriteLock(); }
            OnCollectionChanged();
        }

        void IList.Insert(int index, object value)
        {
            Lock.EnterWriteLock();
            try
            {
                ((IList)_list).Insert(index, value);
            }
            finally { Lock.ExitWriteLock(); }
            OnCollectionChanged();
        }

        public virtual void Insert(int index, T item)
        {
            Lock.EnterWriteLock();
            try
            {
                Debug.Assert(index <= _list.Count);
                _list.Insert(index, item);
            }
            finally { Lock.ExitWriteLock(); }
            OnCollectionChanged();
        }

        public void CopyTo(Array array, int index)
        {
            Lock.EnterWriteLock();
            try
            {
                ((IList)_list).CopyTo(array, index);
            }
            finally { Lock.ExitWriteLock(); }
            OnCollectionChanged();
        }

        public void CopyTo(T[] array, int arrayIndex)
        {
            Lock.EnterWriteLock();
            try
            {
                _list.CopyTo(array, arrayIndex);
            }
            finally { Lock.ExitWriteLock(); }
            OnCollectionChanged();
        }

        public void Clear() => _list.Clear();

        bool IList.Contains(object value)
        {
            Lock.EnterReadLock();
            try
            {
                return ((IList)_list).Contains(value);
            }
            finally { Lock.ExitReadLock(); }
        }

        public bool Contains(T item)
        {
            Lock.EnterReadLock();
            try
            {
                return _list.Contains(item);
            }
            finally { Lock.ExitReadLock(); }
        }


        int IList.IndexOf(object value)
        {
            Lock.EnterReadLock();
            try
            {
                return ((IList)_list).IndexOf(value);
            }
            finally { Lock.ExitReadLock(); }
        }

        public int IndexOf(T item)
        {
            Lock.EnterReadLock();
            try
            {
                return _list.IndexOf(item);
            }
            finally { Lock.ExitReadLock(); }
        }

        object ICollection.SyncRoot
        {
            get
            { Lock.EnterReadLock();
                try
                {
                    return ((ICollection)_list).SyncRoot;
                } 
                finally { Lock.ExitReadLock(); }
            }
        }

        bool ICollection.IsSynchronized
        {
            get
            {
                Lock.EnterReadLock();
                try
                {
                    return ((ICollection) _list).IsSynchronized;
                }
                finally { Lock.ExitReadLock(); }
            }
        }

        bool IList.IsFixedSize
        {
            get
            { Lock.EnterReadLock();
                try
                { return ((IList)_list).IsFixedSize;}
                finally { Lock.ExitReadLock(); }
            }
        }

        bool ICollection<T>.IsReadOnly
        {
            get
            { Lock.EnterReadLock();
                try
                { return ((ICollection<T>)_list).IsReadOnly;}
                finally { Lock.ExitReadLock(); }
            }
        }

        bool IList.IsReadOnly
        {
            get
            { Lock.EnterReadLock();
                try
                { return ((IList)_list).IsReadOnly;}
                finally { Lock.ExitReadLock(); }
            }
        }


        object IList.this[int index]
        {
            get
            {
                Lock.EnterReadLock();
                try
                { return _list[index];}
                finally { Lock.ExitReadLock(); }
            }
            set
            {
                Lock.EnterWriteLock();
                try
                {
                    ((IList)_list)[index] = value;
                }
                finally { Lock.EnterWriteLock();}
            }
        }

        public T this[int index]
        {
            get
            {
                Lock.EnterReadLock();
                try
                {
                    return _list[index];
                }
                finally { Lock.ExitReadLock(); }
            }
            set
            {
                Lock.EnterWriteLock();
                try
                {
                    _list[index] = value;
                }
                finally { Lock.ExitWriteLock(); }
            }
        }

    }
}
