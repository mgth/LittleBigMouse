using System;
using System.Collections.Generic;
using System.Collections.Specialized;
using System.Linq;

namespace Hlab.Mvvm.Observables
{
    public static class ObservableViewModelCollectionExt
    {
        //public static ObservableViewModelCollection<T> SetViewMode<T>(this ObservableViewModelCollection<T> col,  Type viewMode)
        //    where T : INotifyPropertyChanged
        //{
        //    return col.AddCreator( e =>
        //    {
        //        var vm = (T) col.ViewModeContext.GetLinked(e, viewMode);
        //        if (vm == null)
        //        {
        //            vm = (T) col.ViewModeContext.GetLinked(e, viewMode);
        //        }
        //        return vm;
        //    });
        //}
        //public static ObservableViewModelCollection<T> SetViewModeContext<T>(this ObservableViewModelCollection<T> col, ViewModeContext context)
        //    where T : INotifyPropertyChanged
        //{
        //    col.ViewModeContext = context;
        //    return col;
        //}

        public static T SetObserver<T, TT>(this T col, NotifyCollectionChangedEventHandler handler)
            where T : INotifyCollectionChanged, IList<TT>
        {
            if(col is ILockable lckenter) lckenter.Lock.EnterReadLock();
            try
            {
                if (col.Count > 0)
                {
                    handler?.Invoke(null, new NotifyCollectionChangedEventArgs(NotifyCollectionChangedAction.Add, col.ToList()));
                }

                col.CollectionChanged += handler;
                return col;
            }
            finally { if (col is ILockable lckexit) lckexit.Lock.ExitReadLock(); }
        }

        public static T GetOrAdd<T>(this IList<T> col, Func<T, bool> comparator, Func<T> getter)
        {
            var lockable = col as ILockable;

            lockable?.Lock.EnterUpgradeableReadLock();
            try
            {
                foreach (var item in col)
                {
                    if (comparator(item)) return item;
                }

                var newItem = getter();

                col.Add(newItem);

                return newItem;
            }
            finally { lockable?.Lock.ExitUpgradeableReadLock(); }
        }


    }
}
