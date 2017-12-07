using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Diagnostics;
using System.Linq;
using System.Reflection;
using System.Windows;
using Hlab.Base;


////using System.Data.Model;

namespace Hlab.Mvvm
{
    public class MvvmService : Singleton<MvvmService>
    {
        public MvvmService()
        {
            AppDomain currentDomain = AppDomain.CurrentDomain;
            currentDomain.AssemblyLoad += (s, a) => Register(a.LoadedAssembly);


            Register();
        }

        public ViewModeContext MainViewModeContext { get; } = new ViewModeContext("ViewService");

        /// <summary>
        /// Retreive the model type referenced by a viewmodel
        /// </summary>
        /// <param name="viewModelType"></param>
        /// <returns>Model referenced by viewmodel</returns>
        static Type GetModelType(Type viewModelType)
        {
            while (viewModelType != null && viewModelType != typeof(object))
            {
                var cur = viewModelType.IsGenericType ? viewModelType.GetGenericTypeDefinition() : viewModelType;


                foreach(var iface in cur.GetInterfaces())
                {
                    var curiface = iface.IsGenericType ? iface.GetGenericTypeDefinition() : iface;
                    if (curiface != typeof(IViewModel<>)) continue;

                    if (iface.GenericTypeArguments.Length>0)
                        return iface.GenericTypeArguments[0];
                }
                viewModelType = viewModelType.BaseType;
            }
            return null;
        }

        public IEnumerable<Assembly> GetMvvmAssemblies()
        {
            var mvvm = Assembly.GetAssembly(GetType()).GetName();
            return AppDomain.CurrentDomain.GetAssemblies().Where(a => a.GetReferencedAssemblies().Any(e => e.Name == mvvm.Name));
        }

        public IEnumerable<Assembly> GetAllAssemblies()
        {
            return AppDomain.CurrentDomain.GetAssemblies()/*.Where(a => a.GetReferencedAssemblies().Any(e => e.Name == "Mvvm"))*/;
        }

        private readonly ResourceDictionary _dictionnary = new ResourceDictionary();

        private IEnumerable<Type> GetViewClasses(Type type)
        {
            return type.GetInterfaces().Where(i => typeof(IViewClass).IsAssignableFrom(i) && typeof(IViewClass)!=i);
        }

        public void Register()
        {
            foreach (var assembly in GetMvvmAssemblies())
            {
                Register(assembly);
            }
        }


        public void Register(Assembly assembly)
        {
            // Find all user control and register it
 

            //Deal only vith assemblies that reference Mvvm
            var views = assembly.GetTypes().Where(t => typeof(FrameworkElement).IsAssignableFrom(t)).ToList();

            foreach (var viewType in views)
            {
                foreach(var t in viewType.GetInterfaces().Where(i =>
                    i.IsGenericType &&
                    i.GetGenericTypeDefinition() == typeof(IView<,>)
                    ))
//                    foreach (var attr in Attribute.GetCustomAttributes(viewType).OfType<LinkedTo>())
                {
                    var viewMode = t.GetGenericArguments()[0];
                    var viewModelType = t.GetGenericArguments()[1];

                    var classes = GetViewClasses(viewType).ToList();

                    if (classes.Count > 0)
                    {
                        foreach (var cls in classes)
                            Register(viewModelType, viewType, cls, viewMode);

                    }
                    else
                    {
                        Register(viewModelType, viewType, typeof(IViewClassDefault), viewMode);
                    }
                }                        
            }

            Application.Current.Resources.MergedDictionaries.Add(_dictionnary);
        }


        //private readonly List<ModelViewLink> _views = new List<ModelViewLink>();


            /// <summary>
            /// Dict_baseType => dict viewMode => 
            /// </summary>
        private readonly ConcurrentDictionary<Type, ConcurrentDictionary<Type, ConcurrentDictionary<Type, Tuple<Type,Type>>>> _links = new ConcurrentDictionary<Type, ConcurrentDictionary<Type, ConcurrentDictionary<Type, Tuple<Type, Type>>>>();


        //TODO : implement interface
        //TODO : implement viewmode hierarchy
        public Type GetLinkedType(Type baseType, Type viewMode, Type constraint = null)
        {
            if (_links.TryGetValue(baseType, out var dictViewMode) && dictViewMode.TryGetValue(viewMode, out var dictViewClass))
            {
                if (dictViewClass == null) return null;

                Type cls = null;
                Type ret = null;

                var types = constraint == null ? dictViewClass.ToList() : dictViewClass.Where(k => constraint.IsAssignableFrom(k.Key)).ToList();

                foreach (var t in types)
                {
                    if (cls == null || cls.IsSubclassOf(t.Key))
                    {
                        cls = t.Key;
                        ret = t.Value.Item1;
                    }
                }

                return ret;
            }

            return null;
        }

        public void Register(
            Type baseType, 
            Type linkedType, 
            Type viewClass, 
            Type viewMode, 
            Type regFrom=null)
        {
            if (baseType == null)
            {
                Debug.Print(linkedType.FullName + " : linked to null Model");
                return;
            }

            if (baseType!=regFrom)
            {
                foreach (var assembly in GetAllAssemblies())
                {
                    var types = assembly.GetTypes();
                    var viewModels = types.Where(baseType.IsAssignableFrom).ToList();
                    foreach (var vm in viewModels)
                    {
                        Register(vm,linkedType,viewClass,viewMode,baseType);
                    }
                }
                return;
            }

            var dict = _links.GetOrAdd(
                baseType,
                t=>
                {
                    var viewLocatorFactory = new FrameworkElementFactory(typeof(ViewLocator));
                    var template = new DataTemplate(t)
                    {
                        VisualTree = viewLocatorFactory
                    };
                    _dictionnary.Add(new DataTemplateKey(t), template);

                    return new ConcurrentDictionary<Type, ConcurrentDictionary<Type, Tuple<Type, Type>>>();
                });

            var dictClass = dict.GetOrAdd( viewMode, (vm) => new ConcurrentDictionary<Type, Tuple<Type, Type>>());

            var oldLinkedType = dictClass.GetOrAdd(viewClass, c => new Tuple<Type, Type>(linkedType,regFrom));

            if (regFrom == baseType && linkedType!=oldLinkedType.Item1)
            {
                if (oldLinkedType.Item1.IsAssignableFrom(linkedType))
                {
                    dictClass.TryUpdate(viewClass, new Tuple<Type, Type>(linkedType, regFrom), oldLinkedType);
                }
                else
                if (oldLinkedType.Item2.IsAssignableFrom(regFrom))
                {
                    dictClass.TryUpdate(viewClass, new Tuple<Type, Type>(linkedType, regFrom), oldLinkedType);
                }
                else
                throw new InvalidOperationException("View Mode '" + viewMode + "' for \n type : " + baseType.Name + "\n class : " + viewClass + " declared twice \n" + oldLinkedType.Item1.Name + " <> " + linkedType.Name);
            }



            var modelType = GetModelType(baseType);
            if (modelType!=null)
                Register(modelType, baseType,viewClass, viewMode);

        }

        //public void Register(Type modelType, Type viewModelType, Type viewType, string viewMode)
        //{
        //    var isList = viewModelType != null && typeof(ICollection<>).IsAssignableFrom(viewModelType);//viewModel.IsSubclassOf(typeof(IListEntityViewModel)); //;

        //    _views.Add(new ModelViewLink
        //    {
        //        ModelType = modelType,
        //        ViewModelType = viewModelType,
        //        ViewType = viewType,
        //        ViewMode = viewMode,
        //        IsList = isList,
        //    });
        //}

        /// <summary>
        /// Get link from To
        /// </summary>
        /// <param name="type"></param>
        /// <param name="viewMode"></param>
        /// <returns></returns>
        /// 
        //public ModelViewLink GetLink(Type type, string viewMode)
        //{
        //    if (typeof(INotifyPropertyChanged).IsAssignableFrom(type))
        //    {
        //        return GetLinkFromViewModel(type,viewMode);
        //    }

        //    return GetLinkFromModel(type, viewMode);
        //}


        //public ModelViewLink GetLinkFromViewModel(Type vmType, string viewMode)
        //{
        //    var links = _views.Where( v => 
        //        (v.ViewModelType==null || v.ViewModelType.IsAssignableFrom(vmType))
        //        &&
        //        v.ViewMode == viewMode            
        //    );
        //    ModelViewLink link = null;

        //    foreach (var l in links)
        //    {
        //        if (link == null) link = l;
        //        else
        //        {
        //            if (link.ViewModelType!=null && l.ViewModelType!=null &&  l.ViewModelType.IsSubclassOf(link.ViewModelType))
        //                link = l;
        //        }
        //    }

        //    return link;
        //}

        ///// <summary>
        ///// Get link from entity
        ///// </summary>
        ///// <param name="entityType"></param>
        ///// <param name="viewMode"></param>
        ///// <returns></returns>
        //public ModelViewLink GetLinkFromModel(Type entityType, string viewMode)
        //{
        //    var links = _views.Where(v =>
        //       (entityType == v.ModelType || (entityType!=null && v.ModelType!=null && entityType.IsSubclassOf(v.ModelType)))
        //       &&
        //       v.ViewMode == viewMode
        //    );

        //    ModelViewLink link = null;

        //    foreach (var l in links)
        //    {
        //        if (link == null) link = l;
        //        else
        //        {
        //            if (l.ModelType!=null && l.ModelType.IsSubclassOf(link.ModelType))
        //                link = l;
        //        }
        //    }
        //    return link;
        //}

        public IEnumerable<Type> GetViewModes(Type type)
        {
            return _links.Where(e => type == e.Key).SelectMany(e => e.Value.Keys).Distinct();
        }

        //public FrameworkElement GetView(IViewModel vm, string viewMode)
        //{
        //    var link = GetLink(vm?.GetType(), viewMode);
        //    if (link == null) return null;

        //    var view = link.GetView();
        //    view.DataContext = vm;
        //    return view;
        //}

        //public UIElement GetView(ViewModeContext context, IEntity entity, string viewMode)
        //{
        //    if (context == null) context = ApplicationService.D.MainViewModeContext;

        //    var link = GetLink(entity.GetType(), viewMode);
        //    if (link == null) return null;

        //    var viewModel = context.GetLinked(entity,viewMode);

        //    var view = (FrameworkElement)Activator.CreateInstance(link.ViewType);
        //    view.DataContext = viewModel;
        //    return view;
        //}

    }
}
