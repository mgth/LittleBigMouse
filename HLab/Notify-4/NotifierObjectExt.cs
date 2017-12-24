using System;
using System.Collections.Generic;
using System.Linq;
using System.Runtime.CompilerServices;
using System.Text;
using System.Threading.Tasks;

namespace HLab.Notify
{
    public static class NotifierObjectExt
    {
        public static Notifier GetNotifier(this INotifierObject obj)
            => obj.Notifier;

        public static T Get<T>(this INotifierObject n,
            [CallerMemberName] string propertyName = null)
            => n.Notifier.Get<T>(n, old => default(T), propertyName);

        public static T Get<T>(this INotifierObject n, Func<T> getter,
            [CallerMemberName] string propertyName = null)
            => n.Notifier.Get<T>(n, old => getter(), propertyName);

        public static T Get<T>(this INotifierObject n, Func<T, T> getter,
            [CallerMemberName] string propertyName = null)
            => n.Notifier.Get<T>(n, getter, propertyName);

        public static bool Set<T>(this INotifierObject n,
            T value,
            Action<T, T> postUpdateAction,
            [CallerMemberName] string propertyName = null)
            => n.Notifier.Set(n, value, propertyName, postUpdateAction);

        public static bool Set<T>(this INotifierObject n,
            T value,
            [CallerMemberName] string propertyName = null)
            => n.Notifier.Set(n, value, propertyName, null);

        //public static void Subscribe(this INotifierObject n) => n.Notifier.Subscribe(n);
    }
}
