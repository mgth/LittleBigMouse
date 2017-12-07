using System.Collections.Generic;
using System.Windows;
using System.Windows.Media;

namespace Hlab.Base
{
    public static class VisualTreeExt
    {
        public static bool HasChild(this DependencyObject parent, DependencyObject searchchild)
        {
            var childrenCount = VisualTreeHelper.GetChildrenCount(parent);
            for (var i = 0; i < childrenCount; i++)
            {
                var child = VisualTreeHelper.GetChild(parent, i);

                if (child == searchchild) return true;

                if (child.HasChild(searchchild)) return true;
            }

            return false;
        }

        public static T FindVisualParent<T>(this DependencyObject child) where T : DependencyObject
        {
            while (true)
            {
                //get parent item
                var parentObject = VisualTreeHelper.GetParent(child);

                //we've reached the end of the tree
                switch (parentObject)
                {
                    case null:
                        return null;
                    case T parent:
                        return parent;
                }

                //check if the parent matches the type we're looking for

                child = parentObject;
            }
        }

        public static T FindParent<T>(this FrameworkElement child) where T : DependencyObject
        {
            while (true)
            {
                switch (child.Parent)
                {
                    case T parentT:
                        return parentT;
                    case FrameworkElement fe:
                        child = fe;
                        continue;
                    default:
                        return null;
                }
            }
        }

        public static IEnumerable<T> FindChildren<T>(this DependencyObject depObj) where T : DependencyObject
        {
            if (depObj == null) yield break;

            for (var i = 0; i < VisualTreeHelper.GetChildrenCount(depObj); i++)
            {
                var child = VisualTreeHelper.GetChild(depObj, i);
                if (child is T variable)
                {
                    yield return variable;
                }

                foreach (var childOfChild in FindChildren<T>(child))
                {
                    yield return childOfChild;
                }
            }
        }

    }
}
