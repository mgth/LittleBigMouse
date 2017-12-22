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
using System.Collections.Generic;
using System.Collections.Specialized;
using System.Linq;

namespace HLab.Mvvm.Observables
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
