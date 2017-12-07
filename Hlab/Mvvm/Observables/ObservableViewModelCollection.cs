using System;
using System.Collections;
using System.Collections.Specialized;
using System.ComponentModel;
using System.Diagnostics;
using System.Linq;
using System.Runtime.CompilerServices;
using System.Windows.Data;
using Hlab.Notify;

namespace Hlab.Mvvm.Observables
{
    public class ObservableViewModelCollection<T> : ObservableCollectionNotifier<T> , INotifyPropertyChanged
        where T : INotifyPropertyChanged
    {
        private readonly ConditionalWeakTable<Notifier, object> _weakTable= new ConditionalWeakTable<Notifier, object>();
        private Func<object,T> _viewModelCreator = null;
        private Action<CreateHelper> _viewModelDestructor = null;

        public ViewModeContext ViewModeContext
        {
            get => this.Get(() => new ViewModeContext("ObservableViewModelCollection")); set => this.Set(value);
        }

        public ObservableViewModelCollection<T> AddCreator(Func<object, T> c)
        {
            _viewModelCreator = c;
            if (_viewModelCreator == null) return this;
            foreach (var vm in this)
            {
                _viewModelCreator.Invoke(new CreateHelper { List = this, ViewModel = vm });
            }
            return this;
        }
        public ObservableViewModelCollection<T> AddDestructor(Action<CreateHelper> d)
        {
            _viewModelDestructor = d;
            return this;
        }

        public class CreateHelper : IDisposable
        {
            public ObservableViewModelCollection<T> List = null;
            public T ViewModel = default(T);

            public TVm GetViewModel<TVm>() where TVm:class, INotifyPropertyChanged => ViewModel as TVm;

            public bool Done = true;

            public void Dispose()
            {
            }
        }

        private INotifyCollectionChanged _list = null;
        public ObservableViewModelCollection<T> Link(INotifyCollectionChanged list)
        {
            if(_list!=null) _list.CollectionChanged -= _list_CollectionChanged;
            Debug.Assert(Count==0);

            _list = list;

            if (_list != null)
            {
                if (_list is ObservableCollectionNotifier ocn)
                {
                    ocn.SetObserver(_list_CollectionChanged);

                    if (/*_list is IObservableQuery &&*/ _list is INotifyPropertyChanged l)
                    {
                        l.PropertyChanged += L_PropertyChanged;
                    }
                }
                else
                {
                    _list_CollectionChanged(null,
                        new NotifyCollectionChangedEventArgs(NotifyCollectionChangedAction.Add, _list as IList, 0));

                    _list.CollectionChanged += _list_CollectionChanged;

                    if(/*_list is IObservableQuery &&*/ _list is INotifyPropertyChanged l)
                    {
                        l.PropertyChanged += L_PropertyChanged;
                    }                    
                }


            }
            return this;
        }

        private void L_PropertyChanged(object sender, PropertyChangedEventArgs e)
        {
            if (e.PropertyName != "Selected") return;
            if (!(_list is ObservableCollectionNotifier<T> l)) return;


            if (l.Selected == null)
            {
                Selected = default(T);
            }
            else if (_weakTable.TryGetValue(l.Selected.GetNotifier(), out var vm))
            {
                Selected = (T) vm;
            }
        }

        public override T Selected
        {
            get => this.Get(()=>default(T));
            set {
                if (this.Set(Contains(value) ? value : default(T)))
                {
                    if (_list is ObservableCollectionNotifier<T> l) l.Selected = (T)(value as INotifyPropertyChanged)?.GetModel();
                }
            }
        }

        public bool Select(T entity)
        {
            var vm = this.FirstOrDefault(e => (e as INotifyPropertyChanged).Equals(entity));
            if (vm == null) return false;
            Selected = vm;
            return true;
        }

        public ObservableViewModelCollection<T> SetViewMode(Type viewMode)
        {
            return AddCreator(e =>
            {
                // TODO : inplement viewClass
                var vm = (T)ViewModeContext.GetLinked(e, viewMode, null);
                if (vm == null) throw new ArgumentException(e.GetType().Name + " - " + viewMode.Name);
                return vm;
            });
        }

        public ObservableViewModelCollection<T> SetViewMode<TViewMode>() => SetViewMode(typeof(TViewMode));

        public ObservableViewModelCollection<T> SetViewModeContext(ViewModeContext context)
        {
            ViewModeContext = context;
            return this;
        }

        //public void Update()
        //{
        //    (_list as ObservableCollectionNotifier<T>)?.Update();
        //}

        public ObservableViewModelCollection()
        {
            BindingOperations.EnableCollectionSynchronization(this, Lock);
        }

        private void _list_CollectionChanged(object sender, NotifyCollectionChangedEventArgs e)
        {
            switch (e.Action)
            {
                case NotifyCollectionChangedAction.Add:
                    Debug.Assert(e.NewItems != null);
                    Debug.Assert(e.NewItems.Count == (_list as IList)?.Count - Count);
                    {
                        int iNew = e.NewStartingIndex;
                        foreach (var item in e.NewItems)
                        {
                            if (item is INotifyPropertyChanged entity)
                            {
                                var vm = _viewModelCreator.Invoke(entity);
                                _weakTable.Add(entity.GetNotifier(), vm);
                                Insert(iNew, (T)vm);
                            }
                            iNew++;
                        }
                    }
                     break;
                case NotifyCollectionChangedAction.Remove:

                    Debug.Assert(e.OldItems!=null);
                    {
                        int iOld = e.OldStartingIndex;
                        foreach (var item in e.OldItems)
                        {
                            if (!_weakTable.TryGetValue(item.GetNotifier(), out object vm)) continue;

                            if (ReferenceEquals(vm, Selected)) Selected = default(T);


                            _weakTable.Remove(item.GetNotifier());
                            _viewModelDestructor?.Invoke(new CreateHelper { List = this, ViewModel = (T)vm });
                            Remove((T)vm);
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

            Debug.Assert(Count == (_list as IList)?.Count);
        }
    }
}
