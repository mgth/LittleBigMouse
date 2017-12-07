using System;
using System.ComponentModel;
using System.Linq;
using System.Windows.Threading;

namespace Hlab.Notify
{
    public class NotifierHandler
    {
        private readonly object _target;
        public NotifierHandler(object target)
        {
            _target = target;
        }

        private event PropertyChangedEventHandler PropertyChanged;
        
        public void Add(PropertyChangedEventHandler handler)
        {
            PropertyChanged += handler;
        }

        public bool Remove(PropertyChangedEventHandler handler)
        {
            PropertyChanged -= handler;
            return (PropertyChanged == null);
        }

        private PropertyChangedEventHandler[] GetHandlers()
        {
            return PropertyChanged?.GetInvocationList().OfType<PropertyChangedEventHandler>().ToArray();
        }

        public  void OnPropertyChanged(PropertyChangedEventArgs args)
        {
            var handlers = GetHandlers();
            if (handlers == null) return;

            //Parallel.ForEach(handlers,handler =>
            foreach (var handler in handlers)
            {
                if (handler.Target is DispatcherObject d)
                {
                    d.Dispatcher.BeginInvoke(DispatcherPriority.DataBind,
                        new Action(() => handler(_target, args)));
                }
                else
                {
                    handler(_target, args);
                }
            }
            //);
        }
    }
}