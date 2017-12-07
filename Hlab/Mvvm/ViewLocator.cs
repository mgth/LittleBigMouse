using System;
using System.ComponentModel;
using System.Windows;
using System.Windows.Controls;

namespace Hlab.Mvvm
{
    /// <inheritdoc />
    /// <summary>
    /// Logique d'interaction pour EntityViewLocator.xaml
    /// </summary>
    public class ViewLocator : UserControl
    {
/// <summary>
/// 
/// </summary>
        public static readonly DependencyProperty ViewModeProperty 
            = DependencyProperty.RegisterAttached(
                nameof(ViewMode), 
                typeof(Type), 
                typeof(UIElement), 
                new FrameworkPropertyMetadata(
                    typeof(ViewModeDefault),
                    FrameworkPropertyMetadataOptions.Inherits,
                    OnViewModeChanged
                    ));

        public static readonly DependencyProperty ViewClassProperty
            = DependencyProperty.RegisterAttached(
                nameof(ViewClass),
                typeof(Type),
                typeof(UIElement),
                new FrameworkPropertyMetadata(
                    null,
                    FrameworkPropertyMetadataOptions.Inherits,
                    OnViewClassChanged
                ));

        public static readonly DependencyProperty ViewModeContextProperty
            = DependencyProperty.RegisterAttached(
                nameof(ViewModeContext),
                typeof(ViewModeContext),
                typeof(UIElement),
                new FrameworkPropertyMetadata(
                    MvvmService.D.MainViewModeContext,
                    FrameworkPropertyMetadataOptions.Inherits,
                    OnViewModeContextChanged
                    ));
        public static readonly DependencyProperty ModelProperty
            = DependencyProperty.Register(
                nameof(Model),
                typeof(object),
                typeof(ViewLocator),
                new FrameworkPropertyMetadata(null,FrameworkPropertyMetadataOptions.None,OnModelChanged));
        public static object GetModel(DependencyObject obj)
        {
            return (object)obj.GetValue(ModelProperty);
        }
        public static void SetModel(DependencyObject obj, object value)
        {
            obj.SetValue(ModelProperty, value);
        }
        public static Type GetViewMode(DependencyObject obj)
        {
            return (Type)obj.GetValue(ViewModeProperty);
        }
        public static void SetViewMode(DependencyObject obj, Type value)
        {
            obj.SetValue(ViewModeProperty, value);
        }
        public static Type GetViewClass(DependencyObject obj)
        {
            return (Type)obj.GetValue(ViewClassProperty);
        }
        public static void SetViewClass(DependencyObject obj, Type value)
        {
            obj.SetValue(ViewClassProperty, value);
        }
        public static string GetViewModeContext(DependencyObject obj)
        {
            return (string)obj.GetValue(ViewModeProperty);
        }
        public static void SetViewModeContext(DependencyObject obj, ViewModeContext value)
        {
            obj.SetValue(ViewModeProperty, value);
        }
        public object Model
        {
            get => (object)GetValue(ModelProperty);
            set => SetValue(ModelProperty, value);
        }

        public Type ViewMode
        {
            get => (Type)GetValue(ViewModeProperty); set => SetValue(ViewModeProperty, value);
        }
        public Type ViewClass
        {
            get => (Type)GetValue(ViewClassProperty); set => SetValue(ViewClassProperty, value);
        }
        public ViewModeContext ViewModeContext
        {
            get => (ViewModeContext)GetValue(ViewModeContextProperty); set => SetValue(ViewModeContextProperty, value);
        }
        private static void OnModelChanged(DependencyObject dependencyObject,
            DependencyPropertyChangedEventArgs e)
        {
            if (dependencyObject is ViewLocator vl)
                vl.Update(e.OldValue, vl.ViewMode, vl.ViewClass);
        }
        private static void OnViewModeChanged(DependencyObject dependencyObject,
                DependencyPropertyChangedEventArgs e)
        {
            if (dependencyObject is ViewLocator vl)
                vl.Update(vl.Model,e.OldValue as Type, vl.ViewClass);
        }
        private static void OnViewClassChanged(DependencyObject dependencyObject,
            DependencyPropertyChangedEventArgs e)
        {
            if(dependencyObject is ViewLocator vl)
                vl.Update(vl.Model, vl.ViewMode, e.OldValue as Type);
        }
        private static void OnViewModeContextChanged(DependencyObject dependencyObject,
                DependencyPropertyChangedEventArgs e)
        {
            if (dependencyObject is ViewLocator vl)
                vl.Update(vl.Model, vl.ViewMode, vl.ViewClass);
        }

        public ViewLocator()
        {
            this.DataContextChanged += ViewLocator_DataContextChanged;
        }

        private void ViewLocator_DataContextChanged(object sender, DependencyPropertyChangedEventArgs e)
        {
            //TODO : problème dans lbm mais insipensable dans erp
            //if (sender is ViewLocator vl && ReferenceEquals(vl.Model, e.OldValue))
            //{
            //    var oldModel = vl.Model;
            //    vl.Model = e.NewValue;
            //    vl.Update(oldModel, vl.ViewMode, vl.ViewClass);
            //}
        }


        protected void Update(object oldModel, Type oldViewMode, Type oldViewClass)
        {
            if (DesignerProperties.GetIsInDesignMode(this)) return;

            if (Model != null)
            {
                Content = ViewModeContext.GetView(Model, ViewMode, ViewClass);
                return;
            }

            //if (DataContext != null)
            //{
            //    Content = ViewModeContext.GetView(DataContext, ViewMode, ViewClass);
            //    return;
            //}

            Content = null;
        }
    }
}
