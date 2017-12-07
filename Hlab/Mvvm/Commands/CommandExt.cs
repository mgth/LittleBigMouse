using System;
using System.ComponentModel;
using System.Runtime.CompilerServices;
using Hlab.Notify;

namespace Hlab.Mvvm.Commands
{
    public static  class CommandServiceExt
    {
        public static ModelCommand GetCommand(
            this INotifyPropertyChanged notifier, 
            Action execute, 
            Func<bool> enabled,
             [CallerMemberName] string propertyName = null)
        {
            return notifier.Get(()=>
            {
                var ret = new ModelCommand(execute,enabled);
                return ret;
            },propertyName);
        }

        public static ModelCommand GetCommand(
            this INotifyPropertyChanged notifier, 
            Action<object> execute,
            Func<bool> enabled,
            [CallerMemberName] string propertyName = null)
        {
            return notifier.Get(() =>
            {
                var ret = new ModelCommand(execute,enabled);
                return ret;
            }, propertyName);
        }

    }
}
