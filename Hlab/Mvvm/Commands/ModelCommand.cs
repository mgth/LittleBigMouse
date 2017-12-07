using System;
using System.ComponentModel;
using System.Linq;
using System.Windows;
using System.Windows.Input;
using System.Windows.Threading;
using Hlab.Notify;

namespace Hlab.Mvvm.Commands
{
    public class ModelCommand : ICommand, ITriggable
    {
        private readonly Action<object> _execute1 = null;
        private readonly Action _execute0 = null;
        private readonly Func<bool> _canExecuteFunc;
        private bool _canExecute = false;

        public ModelCommand(Action<object> execute, Func<bool> canExecuteFunc)
        {
            _canExecuteFunc = canExecuteFunc;
            _execute1 = execute;
            _canExecute= _canExecuteFunc();
        }
        public ModelCommand(Action execute, Func<bool> canExecuteFunc)
        {
            _canExecuteFunc = canExecuteFunc;
            _execute0 = execute;
            _canExecute = _canExecuteFunc();
        }


        public void SetCanExecute(bool value)
        {
            if (_canExecute == value) return;

            _canExecute = value;
            var handlers = CanExecuteChanged?.GetInvocationList().OfType<EventHandler>().ToArray();
            if (handlers == null) return;

            //Parallel.ForEach(handlers, (handler) =>
            foreach (var handler in handlers)
            {
                var syncer = handler.Target as ISynchronizeInvoke;

                if (syncer != null /*&& !dispatcherObject.CheckAccess()*/)
                {
                    //lock (lockDispatch) toDispatch.Add(handler);
                    //Application.Current.Dispatcher
                    syncer.Invoke(handler, new object[0]);
                    //Invoke(DispatcherPriority.DataBind, handler, this, EventArgs.Empty);
                }
                else
                    Application.Current.Dispatcher.Invoke(DispatcherPriority.DataBind, handler, this,
                        EventArgs.Empty);
                //handler(this, EventArgs.Empty); // note : this does not execute handler in target thread's context
            }
            //CanExecuteChanged?.Invoke(this, EventArgs.Empty);
        }

        public bool CanExecute(object parameter)
        {
            return _canExecute;
        }


        public void Execute(object parameter)
        {
            _execute0?.Invoke();
            _execute1?.Invoke(parameter);
        }

        public event EventHandler CanExecuteChanged;
        public void OnTrigged()
        {
            SetCanExecute(_canExecuteFunc());
        }
    }
}
