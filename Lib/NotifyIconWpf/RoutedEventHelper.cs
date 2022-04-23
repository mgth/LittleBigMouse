using System;
using System.Windows;

namespace Hardcodet.Wpf.TaskbarNotification
{
    /// <summary>
    /// Helper class used by routed events of the
    /// <see cref="TaskbarIcon"/> class.
    /// </summary>
    internal static class RoutedEventHelper
    {
        #region RoutedEvent Helper Methods

        /// <summary>
        /// A static helper method to raise a routed event on a target UIElement or ContentElement.
        /// </summary>
        /// <param name="target">UIElement or ContentElement on which to raise the event</param>
        /// <param name="args">RoutedEventArgs to use when raising the event</param>
        internal static void RaiseEvent(DependencyObject target, RoutedEventArgs args)
        {
            if (target is UIElement uiElement)
            {
                uiElement.RaiseEvent(args);
            }
            else if (target is ContentElement contentElement)
            {
                contentElement.RaiseEvent(args);
            }
        }

        /// <summary>
        /// A static helper method that adds a handler for a routed event 
        /// to a target UIElement or ContentElement.
        /// </summary>
        /// <param name="element">UIElement or ContentElement that listens to the event</param>
        /// <param name="routedEvent">Event that will be handled</param>
        /// <param name="handler">Event handler to be added</param>
        internal static void AddHandler(DependencyObject element, RoutedEvent routedEvent, Delegate handler)
        {
            if (element is UIElement uie)
            {
                uie.AddHandler(routedEvent, handler);
            }
            else if (element is ContentElement ce)
            {
                ce.AddHandler(routedEvent, handler);
            }
        }

        /// <summary>
        /// A static helper method that removes a handler for a routed event 
        /// from a target UIElement or ContentElement.
        /// </summary>
        /// <param name="element">UIElement or ContentElement that listens to the event</param>
        /// <param name="routedEvent">Event that will no longer be handled</param>
        /// <param name="handler">Event handler to be removed</param>
        internal static void RemoveHandler(DependencyObject element, RoutedEvent routedEvent, Delegate handler)
        {
            if (element is UIElement uie)
            {
                uie.RemoveHandler(routedEvent, handler);
            }
            else if (element is ContentElement ce)
            {
                ce.RemoveHandler(routedEvent, handler);
            }
        }

        #endregion
    }
}