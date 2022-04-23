// hardcodet.net NotifyIcon for WPF
// Copyright (c) 2009 - 2013 Philipp Sumi
// Contact and Information: http://www.hardcodet.net
//
// This library is free software; you can redistribute it and/or
// modify it under the terms of the Code Project Open License (CPOL);
// either version 1.0 of the License, or (at your option) any later
// version.
// 
// The above copyright notice and this permission notice shall be
// included in all copies or substantial portions of the Software.
// 
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES
// OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
// NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT
// HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY,
// WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
// FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR
// OTHER DEALINGS IN THE SOFTWARE.
//
// THIS COPYRIGHT NOTICE MAY NOT BE REMOVED FROM THIS FILE


using System;
using System.ComponentModel;
using System.Drawing;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Controls.Primitives;
using System.Windows.Input;
using System.Windows.Media;
using Hardcodet.Wpf.TaskbarNotification.Interop;

namespace Hardcodet.Wpf.TaskbarNotification
{
    /// <summary>
    /// Contains declarations of WPF dependency properties
    /// and events.
    /// </summary>
    partial class TaskbarIcon
    {
        /// <summary>
        /// Category name that is set on designer properties.
        /// </summary>
        public const string CategoryName = "NotifyIcon";


        //POPUP CONTROLS

        #region TrayPopupResolved

        /// <summary>
        /// TrayPopupResolved Read-Only Dependency Property
        /// </summary>
        private static readonly DependencyPropertyKey TrayPopupResolvedPropertyKey
            = DependencyProperty.RegisterReadOnly(nameof(TrayPopupResolved), typeof (Popup), typeof (TaskbarIcon),
                new FrameworkPropertyMetadata(null));


        /// <summary>
        /// A read-only dependency property that returns the <see cref="Popup"/>
        /// that is being displayed in the taskbar area based on a user action.
        /// </summary>
        public static readonly DependencyProperty TrayPopupResolvedProperty
            = TrayPopupResolvedPropertyKey.DependencyProperty;

        /// <summary>
        /// Gets the TrayPopupResolved property. Returns
        /// a <see cref="Popup"/> which is either the
        /// <see cref="TrayPopup"/> control itself or a
        /// <see cref="Popup"/> control that contains the
        /// <see cref="TrayPopup"/>.
        /// </summary>
        [Category(CategoryName)]
        public Popup TrayPopupResolved
        {
            get { return (Popup) GetValue(TrayPopupResolvedProperty); }
        }

        /// <summary>
        /// Provides a secure method for setting the TrayPopupResolved property.  
        /// This dependency property indicates ....
        /// </summary>
        /// <param name="value">The new value for the property.</param>
        protected void SetTrayPopupResolved(Popup value)
        {
            SetValue(TrayPopupResolvedPropertyKey, value);
        }

        #endregion

        #region TrayToolTipResolved

        /// <summary>
        /// TrayToolTipResolved Read-Only Dependency Property
        /// </summary>
        private static readonly DependencyPropertyKey TrayToolTipResolvedPropertyKey
            = DependencyProperty.RegisterReadOnly(nameof(TrayToolTipResolved), typeof (ToolTip), typeof (TaskbarIcon),
                new FrameworkPropertyMetadata(null));


        /// <summary>
        /// A read-only dependency property that returns the <see cref="ToolTip"/>
        /// that is being displayed.
        /// </summary>
        public static readonly DependencyProperty TrayToolTipResolvedProperty
            = TrayToolTipResolvedPropertyKey.DependencyProperty;

        /// <summary>
        /// Gets the TrayToolTipResolved property. Returns 
        /// a <see cref="ToolTip"/> control that was created
        /// in order to display either <see cref="TrayToolTip"/>
        /// or <see cref="ToolTipText"/>.
        /// </summary>
        [Category(CategoryName)]
        [Browsable(true)]
        [Bindable(true)]
        public ToolTip TrayToolTipResolved
        {
            get { return (ToolTip) GetValue(TrayToolTipResolvedProperty); }
        }

        /// <summary>
        /// Provides a secure method for setting the <see cref="TrayToolTipResolved"/>
        /// property.  
        /// </summary>
        /// <param name="value">The new value for the property.</param>
        protected void SetTrayToolTipResolved(ToolTip value)
        {
            SetValue(TrayToolTipResolvedPropertyKey, value);
        }

        #endregion

        #region CustomBalloon

        /// <summary>
        /// CustomBalloon Read-Only Dependency Property
        /// </summary>
        private static readonly DependencyPropertyKey CustomBalloonPropertyKey
            = DependencyProperty.RegisterReadOnly(nameof(CustomBalloon), typeof (Popup), typeof (TaskbarIcon),
                new FrameworkPropertyMetadata(null));

        /// <summary>
        /// Maintains a currently displayed custom balloon.
        /// </summary>
        public static readonly DependencyProperty CustomBalloonProperty
            = CustomBalloonPropertyKey.DependencyProperty;

        /// <summary>
        /// A custom popup that is being displayed in the tray area in order
        /// to display messages to the user.
        /// </summary>
        public Popup CustomBalloon
        {
            get { return (Popup) GetValue(CustomBalloonProperty); }
        }

        /// <summary>
        /// Provides a secure method for setting the <see cref="CustomBalloon"/> property.  
        /// </summary>
        /// <param name="value">The new value for the property.</param>
        protected void SetCustomBalloon(Popup value)
        {
            SetValue(CustomBalloonPropertyKey, value);
        }

        #endregion

        //DEPENDENCY PROPERTIES

        #region Icon property / IconSource dependency property

        private Icon icon;

        /// <summary>
        /// Gets or sets the icon to be displayed. This is not a
        /// dependency property - if you want to assign the property
        /// through XAML, please use the <see cref="IconSource"/>
        /// dependency property.
        /// </summary>
        [Browsable(false)]
        public Icon Icon
        {
            get { return icon; }
            set
            {
                icon = value;
                iconData.IconHandle = value == null ? IntPtr.Zero : icon.Handle;

                Util.WriteIconData(ref iconData, NotifyCommand.Modify, IconDataMembers.Icon);
            }
        }


        /// <summary>
        /// Resolves an image source and updates the <see cref="Icon" /> property accordingly.
        /// </summary>
        public static readonly DependencyProperty IconSourceProperty =
            DependencyProperty.Register(nameof(IconSource),
                typeof (ImageSource),
                typeof (TaskbarIcon),
                new FrameworkPropertyMetadata(null, IconSourcePropertyChanged));

        /// <summary>
        /// A property wrapper for the <see cref="IconSourceProperty"/>
        /// dependency property:<br/>
        /// Resolves an image source and updates the <see cref="Icon" /> property accordingly.
        /// </summary>
        [Category(CategoryName)]
        [Description("Sets the displayed taskbar icon.")]
        public ImageSource IconSource
        {
            get { return (ImageSource) GetValue(IconSourceProperty); }
            set { SetValue(IconSourceProperty, value); }
        }


        /// <summary>
        /// A static callback listener which is being invoked if the
        /// <see cref="IconSourceProperty"/> dependency property has
        /// been changed. Invokes the <see cref="OnIconSourcePropertyChanged"/>
        /// instance method of the changed instance.
        /// </summary>
        /// <param name="d">The currently processed owner of the property.</param>
        /// <param name="e">Provides information about the updated property.</param>
        private static void IconSourcePropertyChanged(DependencyObject d, DependencyPropertyChangedEventArgs e)
        {
            TaskbarIcon owner = (TaskbarIcon) d;
            owner.OnIconSourcePropertyChanged(e);
        }


        /// <summary>
        /// Handles changes of the <see cref="IconSourceProperty"/> dependency property. As
        /// WPF internally uses the dependency property system and bypasses the
        /// <see cref="IconSource"/> property wrapper, updates of the property's value
        /// should be handled here.
        /// </summary>
        /// <param name="e">Provides information about the updated property.</param>
        private void OnIconSourcePropertyChanged(DependencyPropertyChangedEventArgs e)
        {
            ImageSource newValue = (ImageSource) e.NewValue;

            //resolving the ImageSource at design time is unlikely to work
            if (!Util.IsDesignMode) Icon = newValue.ToIcon();
        }

        #endregion

        #region ToolTipText dependency property

        /// <summary>
        /// A tooltip text that is being displayed if no custom <see cref="ToolTip"/>
        /// was set or if custom tooltips are not supported.
        /// </summary>
        public static readonly DependencyProperty ToolTipTextProperty =
            DependencyProperty.Register(nameof(ToolTipText),
                typeof (string),
                typeof (TaskbarIcon),
                new FrameworkPropertyMetadata(string.Empty, ToolTipTextPropertyChanged));


        /// <summary>
        /// A property wrapper for the <see cref="ToolTipTextProperty"/>
        /// dependency property:<br/>
        /// A tooltip text that is being displayed if no custom <see cref="ToolTip"/>
        /// was set or if custom tooltips are not supported.
        /// </summary>
        [Category(CategoryName)]
        [Description("Alternative to a fully blown ToolTip, which is only displayed on Vista and above.")]
        public string ToolTipText
        {
            get { return (string) GetValue(ToolTipTextProperty); }
            set { SetValue(ToolTipTextProperty, value); }
        }


        /// <summary>
        /// A static callback listener which is being invoked if the
        /// <see cref="ToolTipTextProperty"/> dependency property has
        /// been changed. Invokes the <see cref="OnToolTipTextPropertyChanged"/>
        /// instance method of the changed instance.
        /// </summary>
        /// <param name="d">The currently processed owner of the property.</param>
        /// <param name="e">Provides information about the updated property.</param>
        private static void ToolTipTextPropertyChanged(DependencyObject d, DependencyPropertyChangedEventArgs e)
        {
            TaskbarIcon owner = (TaskbarIcon) d;
            owner.OnToolTipTextPropertyChanged(e);
        }


        /// <summary>
        /// Handles changes of the <see cref="ToolTipTextProperty"/> dependency property. As
        /// WPF internally uses the dependency property system and bypasses the
        /// <see cref="ToolTipText"/> property wrapper, updates of the property's value
        /// should be handled here.
        /// </summary>
        /// <param name="e">Provides information about the updated property.</param>
        private void OnToolTipTextPropertyChanged(DependencyPropertyChangedEventArgs e)
        {
            //do not touch tooltips if we have a custom tooltip element
            if (TrayToolTip == null)
            {
                ToolTip currentToolTip = TrayToolTipResolved;
                if (currentToolTip == null)
                {
                    //if we don't have a wrapper tooltip for the tooltip text, create it now
                    CreateCustomToolTip();
                }
                else
                {
                    //if we have a wrapper tooltip that shows the old tooltip text, just update content
                    currentToolTip.Content = e.NewValue;
                }
            }

            WriteToolTipSettings();
        }

        #endregion

        #region TrayToolTip dependency property

        /// <summary>
        /// A custom UI element that is displayed as a tooltip if the user hovers over the taskbar icon.
        /// Works only with Vista and above. Accordingly, you should make sure that
        /// the <see cref="ToolTipText"/> property is set as well.
        /// </summary>
        public static readonly DependencyProperty TrayToolTipProperty =
            DependencyProperty.Register(nameof(TrayToolTip),
                typeof (UIElement),
                typeof (TaskbarIcon),
                new FrameworkPropertyMetadata(null, TrayToolTipPropertyChanged));

        /// <summary>
        /// A property wrapper for the <see cref="TrayToolTipProperty"/>
        /// dependency property:<br/>
        /// A custom UI element that is displayed as a tooltip if the user hovers over the taskbar icon.
        /// Works only with Vista and above. Accordingly, you should make sure that
        /// the <see cref="ToolTipText"/> property is set as well.
        /// </summary>
        [Category(CategoryName)]
        [Description("Custom UI element that is displayed as a tooltip. Only on Vista and above")]
        public UIElement TrayToolTip
        {
            get { return (UIElement) GetValue(TrayToolTipProperty); }
            set { SetValue(TrayToolTipProperty, value); }
        }


        /// <summary>
        /// A static callback listener which is being invoked if the
        /// <see cref="TrayToolTipProperty"/> dependency property has
        /// been changed. Invokes the <see cref="OnTrayToolTipPropertyChanged"/>
        /// instance method of the changed instance.
        /// </summary>
        /// <param name="d">The currently processed owner of the property.</param>
        /// <param name="e">Provides information about the updated property.</param>
        private static void TrayToolTipPropertyChanged(DependencyObject d, DependencyPropertyChangedEventArgs e)
        {
            TaskbarIcon owner = (TaskbarIcon) d;
            owner.OnTrayToolTipPropertyChanged(e);
        }


        /// <summary>
        /// Handles changes of the <see cref="TrayToolTipProperty"/> dependency property. As
        /// WPF internally uses the dependency property system and bypasses the
        /// <see cref="TrayToolTip"/> property wrapper, updates of the property's value
        /// should be handled here.
        /// </summary>
        /// <param name="e">Provides information about the updated property.</param>
        private void OnTrayToolTipPropertyChanged(DependencyPropertyChangedEventArgs e)
        {
            //recreate tooltip control
            CreateCustomToolTip();

            if (e.OldValue != null)
            {
                //remove the taskbar icon reference from the previously used element
                SetParentTaskbarIcon((DependencyObject) e.OldValue, null);
            }

            if (e.NewValue != null)
            {
                //set this taskbar icon as a reference to the new tooltip element
                SetParentTaskbarIcon((DependencyObject) e.NewValue, this);
            }

            //update tooltip settings - needed to make sure a string is set, even
            //if the ToolTipText property is not set. Otherwise, the event that
            //triggers tooltip display is never fired.
            WriteToolTipSettings();
        }

        #endregion

        #region TrayPopup dependency property

        /// <summary>
        /// A control that is displayed as a popup when the taskbar icon is clicked.
        /// </summary>
        public static readonly DependencyProperty TrayPopupProperty =
            DependencyProperty.Register(nameof(TrayPopup),
                typeof (UIElement),
                typeof (TaskbarIcon),
                new FrameworkPropertyMetadata(null, TrayPopupPropertyChanged));

        /// <summary>
        /// A property wrapper for the <see cref="TrayPopupProperty"/>
        /// dependency property:<br/>
        /// A control that is displayed as a popup when the taskbar icon is clicked.
        /// </summary>
        [Category(CategoryName)]
        [Description("Displayed as a Popup if the user clicks on the taskbar icon.")]
        public UIElement TrayPopup
        {
            get { return (UIElement) GetValue(TrayPopupProperty); }
            set { SetValue(TrayPopupProperty, value); }
        }


        /// <summary>
        /// A static callback listener which is being invoked if the
        /// <see cref="TrayPopupProperty"/> dependency property has
        /// been changed. Invokes the <see cref="OnTrayPopupPropertyChanged"/>
        /// instance method of the changed instance.
        /// </summary>
        /// <param name="d">The currently processed owner of the property.</param>
        /// <param name="e">Provides information about the updated property.</param>
        private static void TrayPopupPropertyChanged(DependencyObject d, DependencyPropertyChangedEventArgs e)
        {
            TaskbarIcon owner = (TaskbarIcon) d;
            owner.OnTrayPopupPropertyChanged(e);
        }


        /// <summary>
        /// Handles changes of the <see cref="TrayPopupProperty"/> dependency property. As
        /// WPF internally uses the dependency property system and bypasses the
        /// <see cref="TrayPopup"/> property wrapper, updates of the property's value
        /// should be handled here.
        /// </summary>
        /// <param name="e">Provides information about the updated property.</param>
        private void OnTrayPopupPropertyChanged(DependencyPropertyChangedEventArgs e)
        {
            if (e.OldValue != null)
            {
                //remove the taskbar icon reference from the previously used element
                SetParentTaskbarIcon((DependencyObject) e.OldValue, null);
            }


            if (e.NewValue != null)
            {
                //set this taskbar icon as a reference to the new tooltip element
                SetParentTaskbarIcon((DependencyObject) e.NewValue, this);
            }

            //create a pop
            CreatePopup();
        }

        #endregion

        #region MenuActivation dependency property

        /// <summary>
        /// Defines what mouse events display the context menu.
        /// Defaults to <see cref="PopupActivationMode.RightClick"/>.
        /// </summary>
        public static readonly DependencyProperty MenuActivationProperty =
            DependencyProperty.Register(nameof(MenuActivation),
                typeof (PopupActivationMode),
                typeof (TaskbarIcon),
                new FrameworkPropertyMetadata(PopupActivationMode.RightClick));

        /// <summary>
        /// A property wrapper for the <see cref="MenuActivationProperty"/>
        /// dependency property:<br/>
        /// Defines what mouse events display the context menu.
        /// Defaults to <see cref="PopupActivationMode.RightClick"/>.
        /// </summary>
        [Category(CategoryName)]
        [Description("Defines what mouse events display the context menu.")]
        public PopupActivationMode MenuActivation
        {
            get { return (PopupActivationMode) GetValue(MenuActivationProperty); }
            set { SetValue(MenuActivationProperty, value); }
        }

        #endregion

        #region PopupActivation dependency property

        /// <summary>
        /// Defines what mouse events trigger the <see cref="TrayPopup" />.
        /// Default is <see cref="PopupActivationMode.LeftClick" />.
        /// </summary>
        public static readonly DependencyProperty PopupActivationProperty =
            DependencyProperty.Register(nameof(PopupActivation),
                typeof (PopupActivationMode),
                typeof (TaskbarIcon),
                new FrameworkPropertyMetadata(PopupActivationMode.LeftClick));

        /// <summary>
        /// A property wrapper for the <see cref="PopupActivationProperty"/>
        /// dependency property:<br/>
        /// Defines what mouse events trigger the <see cref="TrayPopup" />.
        /// Default is <see cref="PopupActivationMode.LeftClick" />.
        /// </summary>
        [Category(CategoryName)]
        [Description("Defines what mouse events display the TaskbarIconPopup.")]
        public PopupActivationMode PopupActivation
        {
            get { return (PopupActivationMode) GetValue(PopupActivationProperty); }
            set { SetValue(PopupActivationProperty, value); }
        }

        #endregion

        #region Visibility dependency property override

        /// <summary>
        /// A static callback listener which is being invoked if the
        /// <see cref="UIElement.VisibilityProperty"/> dependency property has
        /// been changed. Invokes the <see cref="OnVisibilityPropertyChanged"/>
        /// instance method of the changed instance.
        /// </summary>
        /// <param name="d">The currently processed owner of the property.</param>
        /// <param name="e">Provides information about the updated property.</param>
        private static void VisibilityPropertyChanged(DependencyObject d, DependencyPropertyChangedEventArgs e)
        {
            TaskbarIcon owner = (TaskbarIcon) d;
            owner.OnVisibilityPropertyChanged(e);
        }


        /// <summary>
        /// Handles changes of the <see cref="UIElement.VisibilityProperty"/> dependency property. As
        /// WPF internally uses the dependency property system and bypasses the
        /// <see cref="Visibility"/> property wrapper, updates of the property's value
        /// should be handled here.
        /// </summary>
        /// <param name="e">Provides information about the updated property.</param>
        private void OnVisibilityPropertyChanged(DependencyPropertyChangedEventArgs e)
        {
            Visibility newValue = (Visibility) e.NewValue;

            //update
            if (newValue == Visibility.Visible)
            {
                CreateTaskbarIcon();
            }
            else
            {
                RemoveTaskbarIcon();
            }
        }

        #endregion

        #region DataContext dependency property override / target update

        /// <summary>
        /// Updates the <see cref="FrameworkElement.DataContextProperty"/> of a given
        /// <see cref="FrameworkElement"/>. This method only updates target elements
        /// that do not already have a data context of their own, and either assigns
        /// the <see cref="FrameworkElement.DataContext"/> of the NotifyIcon, or the
        /// NotifyIcon itself, if no data context was assigned at all.
        /// </summary>
        private void UpdateDataContext(FrameworkElement target, object oldDataContextValue, object newDataContextValue)
        {
            //if there is no target or it's data context is determined through a binding
            //of its own, keep it
            if (target == null || target.IsDataContextDataBound()) return;

            //if the target's data context is the NotifyIcon's old DataContext or the NotifyIcon itself,
            //update it
            if (ReferenceEquals(this, target.DataContext) || Equals(oldDataContextValue, target.DataContext))
            {
                //assign own data context, if available. If there is no data
                //context at all, assign NotifyIcon itself.
                target.DataContext = newDataContextValue ?? this;
            }
        }

        /// <summary>
        /// A static callback listener which is being invoked if the
        /// <see cref="FrameworkElement.DataContextProperty"/> dependency property has
        /// been changed. Invokes the <see cref="OnDataContextPropertyChanged"/>
        /// instance method of the changed instance.
        /// </summary>
        /// <param name="d">The currently processed owner of the property.</param>
        /// <param name="e">Provides information about the updated property.</param>
        private static void DataContextPropertyChanged(DependencyObject d, DependencyPropertyChangedEventArgs e)
        {
            TaskbarIcon owner = (TaskbarIcon) d;
            owner.OnDataContextPropertyChanged(e);
        }


        /// <summary>
        /// Handles changes of the <see cref="FrameworkElement.DataContextProperty"/> dependency property. As
        /// WPF internally uses the dependency property system and bypasses the
        /// <see cref="FrameworkElement.DataContext"/> property wrapper, updates of the property's value
        /// should be handled here.
        /// </summary>
        /// <param name="e">Provides information about the updated property.</param>
        private void OnDataContextPropertyChanged(DependencyPropertyChangedEventArgs e)
        {
            object newValue = e.NewValue;
            object oldValue = e.OldValue;

            //replace custom data context for ToolTips, Popup, and
            //ContextMenu
            UpdateDataContext(TrayPopupResolved, oldValue, newValue);
            UpdateDataContext(TrayToolTipResolved, oldValue, newValue);
            UpdateDataContext(ContextMenu, oldValue, newValue);
        }

        #endregion

        #region ContextMenu dependency property override

        /// <summary>
        /// A static callback listener which is being invoked if the
        /// <see cref="FrameworkElement.ContextMenuProperty"/> dependency property has
        /// been changed. Invokes the <see cref="OnContextMenuPropertyChanged"/>
        /// instance method of the changed instance.
        /// </summary>
        /// <param name="d">The currently processed owner of the property.</param>
        /// <param name="e">Provides information about the updated property.</param>
        private static void ContextMenuPropertyChanged(DependencyObject d, DependencyPropertyChangedEventArgs e)
        {
            TaskbarIcon owner = (TaskbarIcon) d;
            owner.OnContextMenuPropertyChanged(e);
        }


        /// <summary>
        /// Releases the old and updates the new <see cref="ContextMenu"/> property
        /// in order to reflect both the NotifyIcon's <see cref="FrameworkElement.DataContext"/>
        /// property and have the <see cref="ParentTaskbarIconProperty"/> assigned.
        /// </summary>
        /// <param name="e">Provides information about the updated property.</param>
        private void OnContextMenuPropertyChanged(DependencyPropertyChangedEventArgs e)
        {
            if (e.OldValue != null)
            {
                //remove the taskbar icon reference from the previously used element
                SetParentTaskbarIcon((DependencyObject) e.OldValue, null);
            }

            if (e.NewValue != null)
            {
                //set this taskbar icon as a reference to the new tooltip element
                SetParentTaskbarIcon((DependencyObject) e.NewValue, this);
            }

            UpdateDataContext((ContextMenu) e.NewValue, null, DataContext);
        }

        #endregion

        #region DoubleClickCommand dependency property

        /// <summary>
        /// Associates a command that is being executed if the tray icon is being
        /// double clicked.
        /// </summary>
        public static readonly DependencyProperty DoubleClickCommandProperty =
            DependencyProperty.Register(nameof(DoubleClickCommand),
                typeof (ICommand),
                typeof (TaskbarIcon),
                new FrameworkPropertyMetadata(null));

        /// <summary>
        /// A property wrapper for the <see cref="DoubleClickCommandProperty"/>
        /// dependency property:<br/>
        /// Associates a command that is being executed if the tray icon is being
        /// double clicked.
        /// </summary>
        [Category(CategoryName)]
        [Description("A command that is being executed if the tray icon is being double-clicked.")]
        public ICommand DoubleClickCommand
        {
            get { return (ICommand) GetValue(DoubleClickCommandProperty); }
            set { SetValue(DoubleClickCommandProperty, value); }
        }

        #endregion

        #region DoubleClickCommandParameter dependency property

        /// <summary>
        /// Command parameter for the <see cref="DoubleClickCommand"/>.
        /// </summary>
        public static readonly DependencyProperty DoubleClickCommandParameterProperty =
            DependencyProperty.Register(nameof(DoubleClickCommandParameter),
                typeof (object),
                typeof (TaskbarIcon),
                new FrameworkPropertyMetadata(null));

        /// <summary>
        /// A property wrapper for the <see cref="DoubleClickCommandParameterProperty"/>
        /// dependency property:<br/>
        /// Command parameter for the <see cref="DoubleClickCommand"/>.
        /// </summary>
        [Category(CategoryName)]
        [Description("Parameter to submit to the DoubleClickCommand when the user double clicks on the NotifyIcon.")]
        public object DoubleClickCommandParameter
        {
            get { return GetValue(DoubleClickCommandParameterProperty); }
            set { SetValue(DoubleClickCommandParameterProperty, value); }
        }

        #endregion

        #region DoubleClickCommandTarget dependency property

        /// <summary>
        /// The target of the command that is fired if the notify icon is double clicked.
        /// </summary>
        public static readonly DependencyProperty DoubleClickCommandTargetProperty =
            DependencyProperty.Register(nameof(DoubleClickCommandTarget),
                typeof (IInputElement),
                typeof (TaskbarIcon),
                new FrameworkPropertyMetadata(null));

        /// <summary>
        /// A property wrapper for the <see cref="DoubleClickCommandTargetProperty"/>
        /// dependency property:<br/>
        /// The target of the command that is fired if the notify icon is double clicked.
        /// </summary>
        [Category(CategoryName)]
        [Description("The target of the command that is fired if the notify icon is double clicked.")]
        public IInputElement DoubleClickCommandTarget
        {
            get { return (IInputElement) GetValue(DoubleClickCommandTargetProperty); }
            set { SetValue(DoubleClickCommandTargetProperty, value); }
        }

        #endregion

        #region LeftClickCommand dependency property

        /// <summary>
        /// Associates a command that is being executed if the tray icon is being
        /// double clicked.
        /// </summary>
        public static readonly DependencyProperty LeftClickCommandProperty =
            DependencyProperty.Register(nameof(LeftClickCommand),
                typeof (ICommand),
                typeof (TaskbarIcon),
                new FrameworkPropertyMetadata(null));

        /// <summary>
        /// A property wrapper for the <see cref="LeftClickCommandProperty"/>
        /// dependency property:<br/>
        /// Associates a command that is being executed if the tray icon is being
        /// left-clicked.
        /// </summary>
        [Category(CategoryName)]
        [Description("A command that is being executed if the tray icon is being left-clicked.")]
        public ICommand LeftClickCommand
        {
            get { return (ICommand) GetValue(LeftClickCommandProperty); }
            set { SetValue(LeftClickCommandProperty, value); }
        }

        #endregion

        #region LeftClickCommandParameter dependency property

        /// <summary>
        /// Command parameter for the <see cref="LeftClickCommand"/>.
        /// </summary>
        public static readonly DependencyProperty LeftClickCommandParameterProperty =
            DependencyProperty.Register(nameof(LeftClickCommandParameter),
                typeof (object),
                typeof (TaskbarIcon),
                new FrameworkPropertyMetadata(null));

        /// <summary>
        /// A property wrapper for the <see cref="LeftClickCommandParameterProperty"/>
        /// dependency property:<br/>
        /// Command parameter for the <see cref="LeftClickCommand"/>.
        /// </summary>
        [Category(CategoryName)]
        [Description("The target of the command that is fired if the notify icon is clicked with the left mouse button."
            )]
        public object LeftClickCommandParameter
        {
            get { return GetValue(LeftClickCommandParameterProperty); }
            set { SetValue(LeftClickCommandParameterProperty, value); }
        }

        #endregion

        #region LeftClickCommandTarget dependency property

        /// <summary>
        /// The target of the command that is fired if the notify icon is clicked.
        /// </summary>
        public static readonly DependencyProperty LeftClickCommandTargetProperty =
            DependencyProperty.Register(nameof(LeftClickCommandTarget),
                typeof (IInputElement),
                typeof (TaskbarIcon),
                new FrameworkPropertyMetadata(null));

        /// <summary>
        /// A property wrapper for the <see cref="LeftClickCommandTargetProperty"/>
        /// dependency property:<br/>
        /// The target of the command that is fired if the notify icon is clicked.
        /// </summary>
        [Category(CategoryName)]
        [Description("The target of the command that is fired if the notify icon is clicked with the left mouse button."
            )]
        public IInputElement LeftClickCommandTarget
        {
            get { return (IInputElement) GetValue(LeftClickCommandTargetProperty); }
            set { SetValue(LeftClickCommandTargetProperty, value); }
        }

        #endregion

        #region NoLeftClickDelay dependency property

        /// <summary>
        /// Set to true to make left clicks work without delay.
        /// </summary>
        public static readonly DependencyProperty NoLeftClickDelayProperty =
            DependencyProperty.Register(nameof(NoLeftClickDelay),
                typeof(bool),
                typeof(TaskbarIcon),
                new FrameworkPropertyMetadata(false));


        /// <summary>
        /// A property wrapper for the <see cref="NoLeftClickDelayProperty"/>
        /// dependency property:<br/>
        /// Set to true to make left clicks work without delay.
        /// </summary>
        [Category(CategoryName)]
        [Description("Set to true to make left clicks work without delay.")]
        public bool NoLeftClickDelay
        {
            get { return (bool)GetValue(NoLeftClickDelayProperty); }
            set { SetValue(NoLeftClickDelayProperty, value); }
        }

        #endregion

        //EVENTS

        #region TrayLeftMouseDown

        /// <summary>
        /// TrayLeftMouseDown Routed Event
        /// </summary>
        public static readonly RoutedEvent TrayLeftMouseDownEvent = EventManager.RegisterRoutedEvent(
            "TrayLeftMouseDown",
            RoutingStrategy.Bubble, typeof (RoutedEventHandler), typeof (TaskbarIcon));

        /// <summary>
        /// Occurs when the user presses the left mouse button.
        /// </summary>
        [Category(CategoryName)]
        public event RoutedEventHandler TrayLeftMouseDown
        {
            add { AddHandler(TrayLeftMouseDownEvent, value); }
            remove { RemoveHandler(TrayLeftMouseDownEvent, value); }
        }

        /// <summary>
        /// A helper method to raise the TrayLeftMouseDown event.
        /// </summary>
        protected RoutedEventArgs RaiseTrayLeftMouseDownEvent()
        {
            RoutedEventArgs args = RaiseTrayLeftMouseDownEvent(this);
            return args;
        }

        /// <summary>
        /// A static helper method to raise the TrayLeftMouseDown event on a target element.
        /// </summary>
        /// <param name="target">UIElement or ContentElement on which to raise the event</param>
        internal static RoutedEventArgs RaiseTrayLeftMouseDownEvent(DependencyObject target)
        {
            if (target == null) return null;

            RoutedEventArgs args = new RoutedEventArgs(TrayLeftMouseDownEvent);
            RoutedEventHelper.RaiseEvent(target, args);
            return args;
        }

        #endregion

        #region TrayRightMouseDown

        /// <summary>
        /// TrayRightMouseDown Routed Event
        /// </summary>
        public static readonly RoutedEvent TrayRightMouseDownEvent =
            EventManager.RegisterRoutedEvent("TrayRightMouseDown",
                RoutingStrategy.Bubble, typeof (RoutedEventHandler), typeof (TaskbarIcon));

        /// <summary>
        /// Occurs when the presses the right mouse button.
        /// </summary>
        public event RoutedEventHandler TrayRightMouseDown
        {
            add { AddHandler(TrayRightMouseDownEvent, value); }
            remove { RemoveHandler(TrayRightMouseDownEvent, value); }
        }

        /// <summary>
        /// A helper method to raise the TrayRightMouseDown event.
        /// </summary>
        protected RoutedEventArgs RaiseTrayRightMouseDownEvent()
        {
            return RaiseTrayRightMouseDownEvent(this);
        }

        /// <summary>
        /// A static helper method to raise the TrayRightMouseDown event on a target element.
        /// </summary>
        /// <param name="target">UIElement or ContentElement on which to raise the event</param>
        internal static RoutedEventArgs RaiseTrayRightMouseDownEvent(DependencyObject target)
        {
            if (target == null) return null;

            RoutedEventArgs args = new RoutedEventArgs(TrayRightMouseDownEvent);
            RoutedEventHelper.RaiseEvent(target, args);
            return args;
        }

        #endregion

        #region TrayMiddleMouseDown

        /// <summary>
        /// TrayMiddleMouseDown Routed Event
        /// </summary>
        public static readonly RoutedEvent TrayMiddleMouseDownEvent =
            EventManager.RegisterRoutedEvent("TrayMiddleMouseDown",
                RoutingStrategy.Bubble, typeof (RoutedEventHandler), typeof (TaskbarIcon));

        /// <summary>
        /// Occurs when the user presses the middle mouse button.
        /// </summary>
        public event RoutedEventHandler TrayMiddleMouseDown
        {
            add { AddHandler(TrayMiddleMouseDownEvent, value); }
            remove { RemoveHandler(TrayMiddleMouseDownEvent, value); }
        }

        /// <summary>
        /// A helper method to raise the TrayMiddleMouseDown event.
        /// </summary>
        protected RoutedEventArgs RaiseTrayMiddleMouseDownEvent()
        {
            return RaiseTrayMiddleMouseDownEvent(this);
        }

        /// <summary>
        /// A static helper method to raise the TrayMiddleMouseDown event on a target element.
        /// </summary>
        /// <param name="target">UIElement or ContentElement on which to raise the event</param>
        internal static RoutedEventArgs RaiseTrayMiddleMouseDownEvent(DependencyObject target)
        {
            if (target == null) return null;

            RoutedEventArgs args = new RoutedEventArgs(TrayMiddleMouseDownEvent);
            RoutedEventHelper.RaiseEvent(target, args);
            return args;
        }

        #endregion

        #region TrayLeftMouseUp

        /// <summary>
        /// TrayLeftMouseUp Routed Event
        /// </summary>
        public static readonly RoutedEvent TrayLeftMouseUpEvent = EventManager.RegisterRoutedEvent("TrayLeftMouseUp",
            RoutingStrategy.Bubble, typeof (RoutedEventHandler), typeof (TaskbarIcon));

        /// <summary>
        /// Occurs when the user releases the left mouse button.
        /// </summary>
        public event RoutedEventHandler TrayLeftMouseUp
        {
            add { AddHandler(TrayLeftMouseUpEvent, value); }
            remove { RemoveHandler(TrayLeftMouseUpEvent, value); }
        }

        /// <summary>
        /// A helper method to raise the TrayLeftMouseUp event.
        /// </summary>
        protected RoutedEventArgs RaiseTrayLeftMouseUpEvent()
        {
            return RaiseTrayLeftMouseUpEvent(this);
        }

        /// <summary>
        /// A static helper method to raise the TrayLeftMouseUp event on a target element.
        /// </summary>
        /// <param name="target">UIElement or ContentElement on which to raise the event</param>
        internal static RoutedEventArgs RaiseTrayLeftMouseUpEvent(DependencyObject target)
        {
            if (target == null) return null;

            RoutedEventArgs args = new RoutedEventArgs(TrayLeftMouseUpEvent);
            RoutedEventHelper.RaiseEvent(target, args);
            return args;
        }

        #endregion

        #region TrayRightMouseUp

        /// <summary>
        /// TrayRightMouseUp Routed Event
        /// </summary>
        public static readonly RoutedEvent TrayRightMouseUpEvent = EventManager.RegisterRoutedEvent("TrayRightMouseUp",
            RoutingStrategy.Bubble, typeof (RoutedEventHandler), typeof (TaskbarIcon));

        /// <summary>
        /// Occurs when the user releases the right mouse button.
        /// </summary>
        public event RoutedEventHandler TrayRightMouseUp
        {
            add { AddHandler(TrayRightMouseUpEvent, value); }
            remove { RemoveHandler(TrayRightMouseUpEvent, value); }
        }

        /// <summary>
        /// A helper method to raise the TrayRightMouseUp event.
        /// </summary>
        protected RoutedEventArgs RaiseTrayRightMouseUpEvent()
        {
            return RaiseTrayRightMouseUpEvent(this);
        }

        /// <summary>
        /// A static helper method to raise the TrayRightMouseUp event on a target element.
        /// </summary>
        /// <param name="target">UIElement or ContentElement on which to raise the event</param>
        internal static RoutedEventArgs RaiseTrayRightMouseUpEvent(DependencyObject target)
        {
            if (target == null) return null;

            RoutedEventArgs args = new RoutedEventArgs(TrayRightMouseUpEvent);
            RoutedEventHelper.RaiseEvent(target, args);
            return args;
        }

        #endregion

        #region TrayMiddleMouseUp

        /// <summary>
        /// TrayMiddleMouseUp Routed Event
        /// </summary>
        public static readonly RoutedEvent TrayMiddleMouseUpEvent = EventManager.RegisterRoutedEvent(
            "TrayMiddleMouseUp",
            RoutingStrategy.Bubble, typeof (RoutedEventHandler), typeof (TaskbarIcon));

        /// <summary>
        /// Occurs when the user releases the middle mouse button.
        /// </summary>
        public event RoutedEventHandler TrayMiddleMouseUp
        {
            add { AddHandler(TrayMiddleMouseUpEvent, value); }
            remove { RemoveHandler(TrayMiddleMouseUpEvent, value); }
        }

        /// <summary>
        /// A helper method to raise the TrayMiddleMouseUp event.
        /// </summary>
        protected RoutedEventArgs RaiseTrayMiddleMouseUpEvent()
        {
            return RaiseTrayMiddleMouseUpEvent(this);
        }

        /// <summary>
        /// A static helper method to raise the TrayMiddleMouseUp event on a target element.
        /// </summary>
        /// <param name="target">UIElement or ContentElement on which to raise the event</param>
        internal static RoutedEventArgs RaiseTrayMiddleMouseUpEvent(DependencyObject target)
        {
            if (target == null) return null;

            RoutedEventArgs args = new RoutedEventArgs(TrayMiddleMouseUpEvent);
            RoutedEventHelper.RaiseEvent(target, args);
            return args;
        }

        #endregion

        #region TrayMouseDoubleClick

        /// <summary>
        /// TrayMouseDoubleClick Routed Event
        /// </summary>
        public static readonly RoutedEvent TrayMouseDoubleClickEvent =
            EventManager.RegisterRoutedEvent("TrayMouseDoubleClick",
                RoutingStrategy.Bubble, typeof (RoutedEventHandler), typeof (TaskbarIcon));

        /// <summary>
        /// Occurs when the user double-clicks the taskbar icon.
        /// </summary>
        public event RoutedEventHandler TrayMouseDoubleClick
        {
            add { AddHandler(TrayMouseDoubleClickEvent, value); }
            remove { RemoveHandler(TrayMouseDoubleClickEvent, value); }
        }

        /// <summary>
        /// A helper method to raise the TrayMouseDoubleClick event.
        /// </summary>
        protected RoutedEventArgs RaiseTrayMouseDoubleClickEvent()
        {
            RoutedEventArgs args = RaiseTrayMouseDoubleClickEvent(this);
            DoubleClickCommand.ExecuteIfEnabled(DoubleClickCommandParameter, DoubleClickCommandTarget ?? this);
            return args;
        }

        /// <summary>
        /// A static helper method to raise the TrayMouseDoubleClick event on a target element.
        /// </summary>
        /// <param name="target">UIElement or ContentElement on which to raise the event</param>
        internal static RoutedEventArgs RaiseTrayMouseDoubleClickEvent(DependencyObject target)
        {
            if (target == null) return null;

            RoutedEventArgs args = new RoutedEventArgs(TrayMouseDoubleClickEvent);
            RoutedEventHelper.RaiseEvent(target, args);
            return args;
        }

        #endregion

        #region TrayMouseMove

        /// <summary>
        /// TrayMouseMove Routed Event
        /// </summary>
        public static readonly RoutedEvent TrayMouseMoveEvent = EventManager.RegisterRoutedEvent("TrayMouseMove",
            RoutingStrategy.Bubble, typeof (RoutedEventHandler), typeof (TaskbarIcon));

        /// <summary>
        /// Occurs when the user moves the mouse over the taskbar icon.
        /// </summary>
        public event RoutedEventHandler TrayMouseMove
        {
            add { AddHandler(TrayMouseMoveEvent, value); }
            remove { RemoveHandler(TrayMouseMoveEvent, value); }
        }

        /// <summary>
        /// A helper method to raise the TrayMouseMove event.
        /// </summary>
        protected RoutedEventArgs RaiseTrayMouseMoveEvent()
        {
            return RaiseTrayMouseMoveEvent(this);
        }

        /// <summary>
        /// A static helper method to raise the TrayMouseMove event on a target element.
        /// </summary>
        /// <param name="target">UIElement or ContentElement on which to raise the event</param>
        internal static RoutedEventArgs RaiseTrayMouseMoveEvent(DependencyObject target)
        {
            if (target == null) return null;

            RoutedEventArgs args = new RoutedEventArgs(TrayMouseMoveEvent);
            RoutedEventHelper.RaiseEvent(target, args);
            return args;
        }

        #endregion

        #region TrayBalloonTipShown

        /// <summary>
        /// TrayBalloonTipShown Routed Event
        /// </summary>
        public static readonly RoutedEvent TrayBalloonTipShownEvent =
            EventManager.RegisterRoutedEvent("TrayBalloonTipShown",
                RoutingStrategy.Bubble, typeof (RoutedEventHandler), typeof (TaskbarIcon));

        /// <summary>
        /// Occurs when a balloon ToolTip is displayed.
        /// </summary>
        public event RoutedEventHandler TrayBalloonTipShown
        {
            add { AddHandler(TrayBalloonTipShownEvent, value); }
            remove { RemoveHandler(TrayBalloonTipShownEvent, value); }
        }

        /// <summary>
        /// A helper method to raise the TrayBalloonTipShown event.
        /// </summary>
        protected RoutedEventArgs RaiseTrayBalloonTipShownEvent()
        {
            return RaiseTrayBalloonTipShownEvent(this);
        }

        /// <summary>
        /// A static helper method to raise the TrayBalloonTipShown event on a target element.
        /// </summary>
        /// <param name="target">UIElement or ContentElement on which to raise the event</param>
        internal static RoutedEventArgs RaiseTrayBalloonTipShownEvent(DependencyObject target)
        {
            if (target == null) return null;

            RoutedEventArgs args = new RoutedEventArgs(TrayBalloonTipShownEvent);
            RoutedEventHelper.RaiseEvent(target, args);
            return args;
        }

        #endregion

        #region TrayBalloonTipClosed

        /// <summary>
        /// TrayBalloonTipClosed Routed Event
        /// </summary>
        public static readonly RoutedEvent TrayBalloonTipClosedEvent =
            EventManager.RegisterRoutedEvent("TrayBalloonTipClosed",
                RoutingStrategy.Bubble, typeof (RoutedEventHandler), typeof (TaskbarIcon));

        /// <summary>
        /// Occurs when a balloon ToolTip was closed.
        /// </summary>
        public event RoutedEventHandler TrayBalloonTipClosed
        {
            add { AddHandler(TrayBalloonTipClosedEvent, value); }
            remove { RemoveHandler(TrayBalloonTipClosedEvent, value); }
        }

        /// <summary>
        /// A helper method to raise the TrayBalloonTipClosed event.
        /// </summary>
        protected RoutedEventArgs RaiseTrayBalloonTipClosedEvent()
        {
            return RaiseTrayBalloonTipClosedEvent(this);
        }

        /// <summary>
        /// A static helper method to raise the TrayBalloonTipClosed event on a target element.
        /// </summary>
        /// <param name="target">UIElement or ContentElement on which to raise the event</param>
        internal static RoutedEventArgs RaiseTrayBalloonTipClosedEvent(DependencyObject target)
        {
            if (target == null) return null;

            RoutedEventArgs args = new RoutedEventArgs(TrayBalloonTipClosedEvent);
            RoutedEventHelper.RaiseEvent(target, args);
            return args;
        }

        #endregion

        #region TrayBalloonTipClicked

        /// <summary>
        /// TrayBalloonTipClicked Routed Event
        /// </summary>
        public static readonly RoutedEvent TrayBalloonTipClickedEvent =
            EventManager.RegisterRoutedEvent("TrayBalloonTipClicked",
                RoutingStrategy.Bubble, typeof (RoutedEventHandler), typeof (TaskbarIcon));

        /// <summary>
        /// Occurs when the user clicks on a balloon ToolTip.
        /// </summary>
        public event RoutedEventHandler TrayBalloonTipClicked
        {
            add { AddHandler(TrayBalloonTipClickedEvent, value); }
            remove { RemoveHandler(TrayBalloonTipClickedEvent, value); }
        }

        /// <summary>
        /// A helper method to raise the TrayBalloonTipClicked event.
        /// </summary>
        protected RoutedEventArgs RaiseTrayBalloonTipClickedEvent()
        {
            return RaiseTrayBalloonTipClickedEvent(this);
        }

        /// <summary>
        /// A static helper method to raise the TrayBalloonTipClicked event on a target element.
        /// </summary>
        /// <param name="target">UIElement or ContentElement on which to raise the event</param>
        internal static RoutedEventArgs RaiseTrayBalloonTipClickedEvent(DependencyObject target)
        {
            if (target == null) return null;

            RoutedEventArgs args = new RoutedEventArgs(TrayBalloonTipClickedEvent);
            RoutedEventHelper.RaiseEvent(target, args);
            return args;
        }

        #endregion

        #region TrayContextMenuOpen (and PreviewTrayContextMenuOpen)

        /// <summary>
        /// TrayContextMenuOpen Routed Event
        /// </summary>
        public static readonly RoutedEvent TrayContextMenuOpenEvent =
            EventManager.RegisterRoutedEvent("TrayContextMenuOpen",
                RoutingStrategy.Bubble, typeof (RoutedEventHandler), typeof (TaskbarIcon));

        /// <summary>
        /// Bubbled event that occurs when the context menu of the taskbar icon is being displayed.
        /// </summary>
        public event RoutedEventHandler TrayContextMenuOpen
        {
            add { AddHandler(TrayContextMenuOpenEvent, value); }
            remove { RemoveHandler(TrayContextMenuOpenEvent, value); }
        }

        /// <summary>
        /// A helper method to raise the TrayContextMenuOpen event.
        /// </summary>
        protected RoutedEventArgs RaiseTrayContextMenuOpenEvent()
        {
            return RaiseTrayContextMenuOpenEvent(this);
        }

        /// <summary>
        /// A static helper method to raise the TrayContextMenuOpen event on a target element.
        /// </summary>
        /// <param name="target">UIElement or ContentElement on which to raise the event</param>
        internal static RoutedEventArgs RaiseTrayContextMenuOpenEvent(DependencyObject target)
        {
            if (target == null) return null;

            RoutedEventArgs args = new RoutedEventArgs(TrayContextMenuOpenEvent);
            RoutedEventHelper.RaiseEvent(target, args);
            return args;
        }

        /// <summary>
        /// PreviewTrayContextMenuOpen Routed Event
        /// </summary>
        public static readonly RoutedEvent PreviewTrayContextMenuOpenEvent =
            EventManager.RegisterRoutedEvent("PreviewTrayContextMenuOpen",
                RoutingStrategy.Tunnel, typeof (RoutedEventHandler), typeof (TaskbarIcon));

        /// <summary>
        /// Tunneled event that occurs when the context menu of the taskbar icon is being displayed.
        /// </summary>
        public event RoutedEventHandler PreviewTrayContextMenuOpen
        {
            add { AddHandler(PreviewTrayContextMenuOpenEvent, value); }
            remove { RemoveHandler(PreviewTrayContextMenuOpenEvent, value); }
        }

        /// <summary>
        /// A helper method to raise the PreviewTrayContextMenuOpen event.
        /// </summary>
        protected RoutedEventArgs RaisePreviewTrayContextMenuOpenEvent()
        {
            return RaisePreviewTrayContextMenuOpenEvent(this);
        }

        /// <summary>
        /// A static helper method to raise the PreviewTrayContextMenuOpen event on a target element.
        /// </summary>
        /// <param name="target">UIElement or ContentElement on which to raise the event</param>
        internal static RoutedEventArgs RaisePreviewTrayContextMenuOpenEvent(DependencyObject target)
        {
            if (target == null) return null;

            RoutedEventArgs args = new RoutedEventArgs(PreviewTrayContextMenuOpenEvent);
            RoutedEventHelper.RaiseEvent(target, args);
            return args;
        }

        #endregion

        #region TrayPopupOpen (and PreviewTrayPopupOpen)

        /// <summary>
        /// TrayPopupOpen Routed Event
        /// </summary>
        public static readonly RoutedEvent TrayPopupOpenEvent = EventManager.RegisterRoutedEvent("TrayPopupOpen",
            RoutingStrategy.Bubble, typeof (RoutedEventHandler), typeof (TaskbarIcon));

        /// <summary>
        /// Bubbled event that occurs when the custom popup is being opened.
        /// </summary>
        public event RoutedEventHandler TrayPopupOpen
        {
            add { AddHandler(TrayPopupOpenEvent, value); }
            remove { RemoveHandler(TrayPopupOpenEvent, value); }
        }

        /// <summary>
        /// A helper method to raise the TrayPopupOpen event.
        /// </summary>
        protected RoutedEventArgs RaiseTrayPopupOpenEvent()
        {
            return RaiseTrayPopupOpenEvent(this);
        }

        /// <summary>
        /// A static helper method to raise the TrayPopupOpen event on a target element.
        /// </summary>
        /// <param name="target">UIElement or ContentElement on which to raise the event</param>
        internal static RoutedEventArgs RaiseTrayPopupOpenEvent(DependencyObject target)
        {
            if (target == null) return null;

            RoutedEventArgs args = new RoutedEventArgs(TrayPopupOpenEvent);
            RoutedEventHelper.RaiseEvent(target, args);
            return args;
        }

        /// <summary>
        /// PreviewTrayPopupOpen Routed Event
        /// </summary>
        public static readonly RoutedEvent PreviewTrayPopupOpenEvent =
            EventManager.RegisterRoutedEvent("PreviewTrayPopupOpen",
                RoutingStrategy.Tunnel, typeof (RoutedEventHandler), typeof (TaskbarIcon));

        /// <summary>
        /// Tunneled event that occurs when the custom popup is being opened.
        /// </summary>
        public event RoutedEventHandler PreviewTrayPopupOpen
        {
            add { AddHandler(PreviewTrayPopupOpenEvent, value); }
            remove { RemoveHandler(PreviewTrayPopupOpenEvent, value); }
        }

        /// <summary>
        /// A helper method to raise the PreviewTrayPopupOpen event.
        /// </summary>
        protected RoutedEventArgs RaisePreviewTrayPopupOpenEvent()
        {
            return RaisePreviewTrayPopupOpenEvent(this);
        }

        /// <summary>
        /// A static helper method to raise the PreviewTrayPopupOpen event on a target element.
        /// </summary>
        /// <param name="target">UIElement or ContentElement on which to raise the event</param>
        internal static RoutedEventArgs RaisePreviewTrayPopupOpenEvent(DependencyObject target)
        {
            if (target == null) return null;

            RoutedEventArgs args = new RoutedEventArgs(PreviewTrayPopupOpenEvent);
            RoutedEventHelper.RaiseEvent(target, args);
            return args;
        }

        #endregion

        #region TrayToolTipOpen (and PreviewTrayToolTipOpen)

        /// <summary>
        /// TrayToolTipOpen Routed Event
        /// </summary>
        public static readonly RoutedEvent TrayToolTipOpenEvent = EventManager.RegisterRoutedEvent("TrayToolTipOpen",
            RoutingStrategy.Bubble, typeof (RoutedEventHandler), typeof (TaskbarIcon));

        /// <summary>
        /// Bubbled event that occurs when the custom ToolTip is being displayed.
        /// </summary>
        public event RoutedEventHandler TrayToolTipOpen
        {
            add { AddHandler(TrayToolTipOpenEvent, value); }
            remove { RemoveHandler(TrayToolTipOpenEvent, value); }
        }

        /// <summary>
        /// A helper method to raise the TrayToolTipOpen event.
        /// </summary>
        protected RoutedEventArgs RaiseTrayToolTipOpenEvent()
        {
            return RaiseTrayToolTipOpenEvent(this);
        }

        /// <summary>
        /// A static helper method to raise the TrayToolTipOpen event on a target element.
        /// </summary>
        /// <param name="target">UIElement or ContentElement on which to raise the event</param>
        internal static RoutedEventArgs RaiseTrayToolTipOpenEvent(DependencyObject target)
        {
            if (target == null) return null;

            RoutedEventArgs args = new RoutedEventArgs(TrayToolTipOpenEvent);
            RoutedEventHelper.RaiseEvent(target, args);
            return args;
        }

        /// <summary>
        /// PreviewTrayToolTipOpen Routed Event
        /// </summary>
        public static readonly RoutedEvent PreviewTrayToolTipOpenEvent =
            EventManager.RegisterRoutedEvent("PreviewTrayToolTipOpen",
                RoutingStrategy.Tunnel, typeof (RoutedEventHandler), typeof (TaskbarIcon));

        /// <summary>
        /// Tunneled event that occurs when the custom ToolTip is being displayed.
        /// </summary>
        public event RoutedEventHandler PreviewTrayToolTipOpen
        {
            add { AddHandler(PreviewTrayToolTipOpenEvent, value); }
            remove { RemoveHandler(PreviewTrayToolTipOpenEvent, value); }
        }

        /// <summary>
        /// A helper method to raise the PreviewTrayToolTipOpen event.
        /// </summary>
        protected RoutedEventArgs RaisePreviewTrayToolTipOpenEvent()
        {
            return RaisePreviewTrayToolTipOpenEvent(this);
        }

        /// <summary>
        /// A static helper method to raise the PreviewTrayToolTipOpen event on a target element.
        /// </summary>
        /// <param name="target">UIElement or ContentElement on which to raise the event</param>
        internal static RoutedEventArgs RaisePreviewTrayToolTipOpenEvent(DependencyObject target)
        {
            if (target == null) return null;

            RoutedEventArgs args = new RoutedEventArgs(PreviewTrayToolTipOpenEvent);
            RoutedEventHelper.RaiseEvent(target, args);
            return args;
        }

        #endregion

        #region TrayToolTipClose (and PreviewTrayToolTipClose)

        /// <summary>
        /// TrayToolTipClose Routed Event
        /// </summary>
        public static readonly RoutedEvent TrayToolTipCloseEvent = EventManager.RegisterRoutedEvent("TrayToolTipClose",
            RoutingStrategy.Bubble, typeof (RoutedEventHandler), typeof (TaskbarIcon));

        /// <summary>
        /// Bubbled event that occurs when a custom tooltip is being closed.
        /// </summary>
        public event RoutedEventHandler TrayToolTipClose
        {
            add { AddHandler(TrayToolTipCloseEvent, value); }
            remove { RemoveHandler(TrayToolTipCloseEvent, value); }
        }

        /// <summary>
        /// A helper method to raise the TrayToolTipClose event.
        /// </summary>
        protected RoutedEventArgs RaiseTrayToolTipCloseEvent()
        {
            return RaiseTrayToolTipCloseEvent(this);
        }

        /// <summary>
        /// A static helper method to raise the TrayToolTipClose event on a target element.
        /// </summary>
        /// <param name="target">UIElement or ContentElement on which to raise the event</param>
        internal static RoutedEventArgs RaiseTrayToolTipCloseEvent(DependencyObject target)
        {
            if (target == null) return null;

            RoutedEventArgs args = new RoutedEventArgs(TrayToolTipCloseEvent);
            RoutedEventHelper.RaiseEvent(target, args);
            return args;
        }

        /// <summary>
        /// PreviewTrayToolTipClose Routed Event
        /// </summary>
        public static readonly RoutedEvent PreviewTrayToolTipCloseEvent =
            EventManager.RegisterRoutedEvent("PreviewTrayToolTipClose",
                RoutingStrategy.Tunnel, typeof (RoutedEventHandler), typeof (TaskbarIcon));

        /// <summary>
        /// Tunneled event that occurs when a custom tooltip is being closed.
        /// </summary>
        public event RoutedEventHandler PreviewTrayToolTipClose
        {
            add { AddHandler(PreviewTrayToolTipCloseEvent, value); }
            remove { RemoveHandler(PreviewTrayToolTipCloseEvent, value); }
        }

        /// <summary>
        /// A helper method to raise the PreviewTrayToolTipClose event.
        /// </summary>
        protected RoutedEventArgs RaisePreviewTrayToolTipCloseEvent()
        {
            return RaisePreviewTrayToolTipCloseEvent(this);
        }

        /// <summary>
        /// A static helper method to raise the PreviewTrayToolTipClose event on a target element.
        /// </summary>
        /// <param name="target">UIElement or ContentElement on which to raise the event</param>
        internal static RoutedEventArgs RaisePreviewTrayToolTipCloseEvent(DependencyObject target)
        {
            if (target == null) return null;

            RoutedEventArgs args = new RoutedEventArgs(PreviewTrayToolTipCloseEvent);
            RoutedEventHelper.RaiseEvent(target, args);
            return args;
        }

        #endregion

        //ATTACHED EVENTS

        #region PopupOpened

        /// <summary>
        /// PopupOpened Attached Routed Event
        /// </summary>
        public static readonly RoutedEvent PopupOpenedEvent = EventManager.RegisterRoutedEvent("PopupOpened",
            RoutingStrategy.Bubble, typeof (RoutedEventHandler), typeof (TaskbarIcon));

        /// <summary>
        /// Adds a handler for the PopupOpened attached event
        /// </summary>
        /// <param name="element">UIElement or ContentElement that listens to the event</param>
        /// <param name="handler">Event handler to be added</param>
        public static void AddPopupOpenedHandler(DependencyObject element, RoutedEventHandler handler)
        {
            RoutedEventHelper.AddHandler(element, PopupOpenedEvent, handler);
        }

        /// <summary>
        /// Removes a handler for the PopupOpened attached event
        /// </summary>
        /// <param name="element">UIElement or ContentElement that listens to the event</param>
        /// <param name="handler">Event handler to be removed</param>
        public static void RemovePopupOpenedHandler(DependencyObject element, RoutedEventHandler handler)
        {
            RoutedEventHelper.RemoveHandler(element, PopupOpenedEvent, handler);
        }

        /// <summary>
        /// A static helper method to raise the PopupOpened event on a target element.
        /// </summary>
        /// <param name="target">UIElement or ContentElement on which to raise the event</param>
        internal static RoutedEventArgs RaisePopupOpenedEvent(DependencyObject target)
        {
            if (target == null) return null;

            RoutedEventArgs args = new RoutedEventArgs(PopupOpenedEvent);
            RoutedEventHelper.RaiseEvent(target, args);
            return args;
        }

        #endregion

        #region ToolTipOpened

        /// <summary>
        /// ToolTipOpened Attached Routed Event
        /// </summary>
        public static readonly RoutedEvent ToolTipOpenedEvent = EventManager.RegisterRoutedEvent("ToolTipOpened",
            RoutingStrategy.Bubble, typeof (RoutedEventHandler), typeof (TaskbarIcon));

        /// <summary>
        /// Adds a handler for the ToolTipOpened attached event
        /// </summary>
        /// <param name="element">UIElement or ContentElement that listens to the event</param>
        /// <param name="handler">Event handler to be added</param>
        public static void AddToolTipOpenedHandler(DependencyObject element, RoutedEventHandler handler)
        {
            RoutedEventHelper.AddHandler(element, ToolTipOpenedEvent, handler);
        }

        /// <summary>
        /// Removes a handler for the ToolTipOpened attached event
        /// </summary>
        /// <param name="element">UIElement or ContentElement that listens to the event</param>
        /// <param name="handler">Event handler to be removed</param>
        public static void RemoveToolTipOpenedHandler(DependencyObject element, RoutedEventHandler handler)
        {
            RoutedEventHelper.RemoveHandler(element, ToolTipOpenedEvent, handler);
        }

        /// <summary>
        /// A static helper method to raise the ToolTipOpened event on a target element.
        /// </summary>
        /// <param name="target">UIElement or ContentElement on which to raise the event</param>
        internal static RoutedEventArgs RaiseToolTipOpenedEvent(DependencyObject target)
        {
            if (target == null) return null;

            RoutedEventArgs args = new RoutedEventArgs(ToolTipOpenedEvent);
            RoutedEventHelper.RaiseEvent(target, args);
            return args;
        }

        #endregion

        #region ToolTipClose

        /// <summary>
        /// ToolTipClose Attached Routed Event
        /// </summary>
        public static readonly RoutedEvent ToolTipCloseEvent = EventManager.RegisterRoutedEvent("ToolTipClose",
            RoutingStrategy.Bubble, typeof (RoutedEventHandler), typeof (TaskbarIcon));

        /// <summary>
        /// Adds a handler for the ToolTipClose attached event
        /// </summary>
        /// <param name="element">UIElement or ContentElement that listens to the event</param>
        /// <param name="handler">Event handler to be added</param>
        public static void AddToolTipCloseHandler(DependencyObject element, RoutedEventHandler handler)
        {
            RoutedEventHelper.AddHandler(element, ToolTipCloseEvent, handler);
        }

        /// <summary>
        /// Removes a handler for the ToolTipClose attached event
        /// </summary>
        /// <param name="element">UIElement or ContentElement that listens to the event</param>
        /// <param name="handler">Event handler to be removed</param>
        public static void RemoveToolTipCloseHandler(DependencyObject element, RoutedEventHandler handler)
        {
            RoutedEventHelper.RemoveHandler(element, ToolTipCloseEvent, handler);
        }

        /// <summary>
        /// A static helper method to raise the ToolTipClose event on a target element.
        /// </summary>
        /// <param name="target">UIElement or ContentElement on which to raise the event</param>
        internal static RoutedEventArgs RaiseToolTipCloseEvent(DependencyObject target)
        {
            if (target == null) return null;

            RoutedEventArgs args = new RoutedEventArgs(ToolTipCloseEvent);
            RoutedEventHelper.RaiseEvent(target, args);
            return args;
        }

        #endregion

        #region BalloonShowing

        /// <summary>
        /// BalloonShowing Attached Routed Event
        /// </summary>
        public static readonly RoutedEvent BalloonShowingEvent = EventManager.RegisterRoutedEvent("BalloonShowing",
            RoutingStrategy.Bubble, typeof (RoutedEventHandler), typeof (TaskbarIcon));

        /// <summary>
        /// Adds a handler for the BalloonShowing attached event
        /// </summary>
        /// <param name="element">UIElement or ContentElement that listens to the event</param>
        /// <param name="handler">Event handler to be added</param>
        public static void AddBalloonShowingHandler(DependencyObject element, RoutedEventHandler handler)
        {
            RoutedEventHelper.AddHandler(element, BalloonShowingEvent, handler);
        }

        /// <summary>
        /// Removes a handler for the BalloonShowing attached event
        /// </summary>
        /// <param name="element">UIElement or ContentElement that listens to the event</param>
        /// <param name="handler">Event handler to be removed</param>
        public static void RemoveBalloonShowingHandler(DependencyObject element, RoutedEventHandler handler)
        {
            RoutedEventHelper.RemoveHandler(element, BalloonShowingEvent, handler);
        }

        /// <summary>
        /// A static helper method to raise the BalloonShowing event on a target element.
        /// </summary>
        /// <param name="target">UIElement or ContentElement on which to raise the event</param>
        /// <param name="source">The <see cref="TaskbarIcon"/> instance that manages the balloon.</param>
        internal static RoutedEventArgs RaiseBalloonShowingEvent(DependencyObject target, TaskbarIcon source)
        {
            if (target == null) return null;

            RoutedEventArgs args = new RoutedEventArgs(BalloonShowingEvent, source);
            RoutedEventHelper.RaiseEvent(target, args);
            return args;
        }

        #endregion

        #region BalloonClosing

        /// <summary>
        /// BalloonClosing Attached Routed Event
        /// </summary>
        public static readonly RoutedEvent BalloonClosingEvent = EventManager.RegisterRoutedEvent("BalloonClosing",
            RoutingStrategy.Bubble, typeof (RoutedEventHandler), typeof (TaskbarIcon));

        /// <summary>
        /// Adds a handler for the BalloonClosing attached event
        /// </summary>
        /// <param name="element">UIElement or ContentElement that listens to the event</param>
        /// <param name="handler">Event handler to be added</param>
        public static void AddBalloonClosingHandler(DependencyObject element, RoutedEventHandler handler)
        {
            RoutedEventHelper.AddHandler(element, BalloonClosingEvent, handler);
        }

        /// <summary>
        /// Removes a handler for the BalloonClosing attached event
        /// </summary>
        /// <param name="element">UIElement or ContentElement that listens to the event</param>
        /// <param name="handler">Event handler to be removed</param>
        public static void RemoveBalloonClosingHandler(DependencyObject element, RoutedEventHandler handler)
        {
            RoutedEventHelper.RemoveHandler(element, BalloonClosingEvent, handler);
        }

        /// <summary>
        /// A static helper method to raise the BalloonClosing event on a target element.
        /// </summary>
        /// <param name="target">UIElement or ContentElement on which to raise the event</param>
        /// <param name="source">The <see cref="TaskbarIcon"/> instance that manages the balloon.</param>
        internal static RoutedEventArgs RaiseBalloonClosingEvent(DependencyObject target, TaskbarIcon source)
        {
            if (target == null) return null;

            RoutedEventArgs args = new RoutedEventArgs(BalloonClosingEvent, source);
            RoutedEventHelper.RaiseEvent(target, args);
            return args;
        }

        #endregion

        //ATTACHED PROPERTIES

        #region ParentTaskbarIcon

        /// <summary>
        /// An attached property that is assigned to displayed UI elements (balloons, tooltips, context menus), and
        /// that can be used to bind to this control. The attached property is being derived, so binding is
        /// quite straightforward:
        /// <code>
        /// <TextBlock Text="{Binding RelativeSource={RelativeSource Self}, Path=(tb:TaskbarIcon.ParentTaskbarIcon).ToolTipText}" />
        /// </code>
        /// </summary>  
        public static readonly DependencyProperty ParentTaskbarIconProperty =
            DependencyProperty.RegisterAttached("ParentTaskbarIcon", typeof (TaskbarIcon), typeof (TaskbarIcon),
                new FrameworkPropertyMetadata(null, FrameworkPropertyMetadataOptions.Inherits));

        /// <summary>
        /// Gets the ParentTaskbarIcon property.  This dependency property 
        /// indicates ....
        /// </summary>
        public static TaskbarIcon GetParentTaskbarIcon(DependencyObject d)
        {
            return (TaskbarIcon) d.GetValue(ParentTaskbarIconProperty);
        }

        /// <summary>
        /// Sets the ParentTaskbarIcon property.  This dependency property 
        /// indicates ....
        /// </summary>
        public static void SetParentTaskbarIcon(DependencyObject d, TaskbarIcon value)
        {
            d.SetValue(ParentTaskbarIconProperty, value);
        }

        #endregion

        //BASE CLASS PROPERTY OVERRIDES

        /// <summary>
        /// Registers properties.
        /// </summary>
        static TaskbarIcon()
        {
            //register change listener for the Visibility property
            var md = new PropertyMetadata(Visibility.Visible, VisibilityPropertyChanged);
            VisibilityProperty.OverrideMetadata(typeof (TaskbarIcon), md);

            //register change listener for the DataContext property
            md = new FrameworkPropertyMetadata(DataContextPropertyChanged);
            DataContextProperty.OverrideMetadata(typeof (TaskbarIcon), md);

            //register change listener for the ContextMenu property
            md = new FrameworkPropertyMetadata(ContextMenuPropertyChanged);
            ContextMenuProperty.OverrideMetadata(typeof (TaskbarIcon), md);
        }
    }
}