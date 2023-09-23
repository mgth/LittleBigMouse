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
using System.Diagnostics;
using System.Drawing;
using System.Threading;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Controls.Primitives;
using System.Windows.Interop;
using System.Windows.Threading;
using Hardcodet.Wpf.TaskbarNotification.Interop;
using Point = Hardcodet.Wpf.TaskbarNotification.Interop.Point;


namespace Hardcodet.Wpf.TaskbarNotification
{
    /// <summary>
    /// A WPF proxy to for a taskbar icon (NotifyIcon) that sits in the system's
    /// taskbar notification area ("system tray").
    /// </summary>
    public partial class TaskbarIcon : FrameworkElement, IDisposable
    {
        private readonly object lockObject = new object();

        #region Members

        /// <summary>
        /// Represents the current icon data.
        /// </summary>
        private NotifyIconData iconData;

        /// <summary>
        /// Receives messages from the taskbar icon.
        /// </summary>
        private readonly WindowMessageSink messageSink;

        /// <summary>
        /// An action that is being invoked if the
        /// <see cref="singleClickTimer"/> fires.
        /// </summary>
        private Action singleClickTimerAction;

        /// <summary>
        /// A timer that is used to differentiate between single
        /// and double clicks.
        /// </summary>
        private readonly Timer singleClickTimer;

        /// <summary>
        /// The time we should wait for a double click.
        /// </summary>
        private int DoubleClickWaitTime => NoLeftClickDelay ? 0 : WinApi.GetDoubleClickTime();

        /// <summary>
        /// A timer that is used to close open balloon tooltips.
        /// </summary>
        private readonly Timer balloonCloseTimer;

        /// <summary>
        /// Indicates whether the taskbar icon has been created or not.
        /// </summary>
        public bool IsTaskbarIconCreated { get; private set; }

        /// <summary>
        /// Indicates whether custom tooltips are supported, which depends
        /// on the OS. Windows Vista or higher is required in order to
        /// support this feature.
        /// </summary>
        public bool SupportsCustomToolTips => messageSink.Version == NotifyIconVersion.Vista;


        /// <summary>
        /// Checks whether a non-tooltip popup is currently opened.
        /// </summary>
        private bool IsPopupOpen
        {
            get
            {
                var popup = TrayPopupResolved;
                var menu = ContextMenu;
                var balloon = CustomBalloon;

                return popup != null && popup.IsOpen ||
                       menu != null && menu.IsOpen ||
                       balloon != null && balloon.IsOpen;
            }
        }

        #endregion

        #region Construction

        /// <summary>
        /// Initializes the taskbar icon and registers a message listener
        /// in order to receive events from the taskbar area.
        /// </summary>
        public TaskbarIcon()
        {
            // using dummy sink in design mode
            messageSink = Util.IsDesignMode
                ? WindowMessageSink.CreateEmpty()
                : new WindowMessageSink(NotifyIconVersion.Win95);

            // init icon data structure
            iconData = NotifyIconData.CreateDefault(messageSink.MessageWindowHandle);

            // create the taskbar icon
            CreateTaskbarIcon();

            // register event listeners
            messageSink.MouseEventReceived += OnMouseEvent;
            messageSink.TaskbarCreated += OnTaskbarCreated;
            messageSink.ChangeToolTipStateRequest += OnToolTipChange;
            messageSink.BalloonToolTipChanged += OnBalloonToolTipChanged;

            // init single click / balloon timers
            singleClickTimer = new Timer(DoSingleClickAction);
            balloonCloseTimer = new Timer(CloseBalloonCallback);

            // register listener in order to get notified when the application closes
            if (Application.Current != null)
            {
                Application.Current.Exit += OnExit;
            }
        }

        #endregion

        #region Custom Balloons
        /// <summary>
        /// A delegate to handle customer popup positions.
        /// </summary>
        public delegate Point GetCustomPopupPosition();

        /// <summary>
        /// Specify a custom popup position
        /// </summary>
        public GetCustomPopupPosition CustomPopupPosition { get; set; }

        /// <summary>
        /// Returns the location of the system tray
        /// </summary>
        /// <returns>Point</returns>
        public Point GetPopupTrayPosition()
        {
            return TrayInfo.GetTrayLocation();
        }

        /// <summary>
        /// Shows a custom control as a tooltip in the tray location.
        /// </summary>
        /// <param name="balloon"></param>
        /// <param name="animation">An optional animation for the popup.</param>
        /// <param name="timeout">The time after which the popup is being closed.
        /// Submit null in order to keep the balloon open indefinitely
        /// </param>
        /// <exception cref="ArgumentNullException">If <paramref name="balloon"/>
        /// is a null reference.</exception>
        public void ShowCustomBalloon(UIElement balloon, PopupAnimation animation, int? timeout)
        {
            var dispatcher = this.GetDispatcher();
            if (!dispatcher.CheckAccess())
            {
                var action = new Action(() => ShowCustomBalloon(balloon, animation, timeout));
                dispatcher.Invoke(DispatcherPriority.Normal, action);
                return;
            }

            if (balloon == null) throw new ArgumentNullException(nameof(balloon));
            if (timeout.HasValue && timeout < 500)
            {
                string msg = "Invalid timeout of {0} milliseconds. Timeout must be at least 500 ms";
                msg = string.Format(msg, timeout);
                throw new ArgumentOutOfRangeException(nameof(timeout), msg);
            }

            EnsureNotDisposed();

            // make sure we don't have an open balloon
            lock (lockObject)
            {
                CloseBalloon();
            }

            // create an invisible popup that hosts the UIElement
            Popup popup = new Popup
            {
                AllowsTransparency = true
            };

            // provide the popup with the taskbar icon's data context
            UpdateDataContext(popup, null, DataContext);

            // don't animate by default - developers can use attached events or override
            popup.PopupAnimation = animation;

            // in case the balloon is cleaned up through routed events, the
            // control didn't remove the balloon from its parent popup when
            // if was closed the last time - just make sure it doesn't have
            // a parent that is a popup
            var parent = LogicalTreeHelper.GetParent(balloon) as Popup;
            if (parent != null) parent.Child = null;

            if (parent != null)
            {
                string msg = "Cannot display control [{0}] in a new balloon popup - that control already has a parent. You may consider creating new balloons every time you want to show one.";
                msg = string.Format(msg, balloon);
                throw new InvalidOperationException(msg);
            }

            popup.Child = balloon;

            //don't set the PlacementTarget as it causes the popup to become hidden if the
            //TaskbarIcon's parent is hidden, too...
            //popup.PlacementTarget = this;

            popup.Placement = PlacementMode.AbsolutePoint;
            popup.StaysOpen = true;


            Point position = CustomPopupPosition != null ? CustomPopupPosition() : GetPopupTrayPosition();
            popup.HorizontalOffset = position.X - 1;
            popup.VerticalOffset = position.Y - 1;

            //store reference
            lock (lockObject)
            {
                SetCustomBalloon(popup);
            }

            // assign this instance as an attached property
            SetParentTaskbarIcon(balloon, this);

            // fire attached event
            RaiseBalloonShowingEvent(balloon, this);

            // display item
            popup.IsOpen = true;

            if (timeout.HasValue)
            {
                // register timer to close the popup
                balloonCloseTimer.Change(timeout.Value, Timeout.Infinite);
            }
        }


        /// <summary>
        /// Resets the closing timeout, which effectively
        /// keeps a displayed balloon message open until
        /// it is either closed programmatically through
        /// <see cref="CloseBalloon"/> or due to a new
        /// message being displayed.
        /// </summary>
        public void ResetBalloonCloseTimer()
        {
            if (IsDisposed) return;

            lock (lockObject)
            {
                //reset timer in any case
                balloonCloseTimer.Change(Timeout.Infinite, Timeout.Infinite);
            }
        }


        /// <summary>
        /// Closes the current <see cref="CustomBalloon"/>, if the
        /// property is set.
        /// </summary>
        public void CloseBalloon()
        {
            if (IsDisposed) return;

            Dispatcher dispatcher = this.GetDispatcher();
            if (!dispatcher.CheckAccess())
            {
                Action action = CloseBalloon;
                dispatcher.Invoke(DispatcherPriority.Normal, action);
                return;
            }

            lock (lockObject)
            {
                // reset timer in any case
                balloonCloseTimer.Change(Timeout.Infinite, Timeout.Infinite);

                // reset old popup, if we still have one
                Popup popup = CustomBalloon;
                if (popup == null)
                {
                    return;
                }

                UIElement element = popup.Child;

                // announce closing
                RoutedEventArgs eventArgs = RaiseBalloonClosingEvent(element, this);
                if (!eventArgs.Handled)
                {
                    // if the event was handled, clear the reference to the popup,
                    // but don't close it - the handling code has to manage this stuff now

                    // close the popup
                    popup.IsOpen = false;

                    // remove the reference of the popup to the balloon in case we want to reuse
                    // the balloon (then added to a new popup)
                    popup.Child = null;

                    // reset attached property
                    if (element != null) SetParentTaskbarIcon(element, null);
                }

                // remove custom balloon anyway
                SetCustomBalloon(null);
            }
        }


        /// <summary>
        /// Timer-invoke event which closes the currently open balloon and
        /// resets the <see cref="CustomBalloon"/> dependency property.
        /// </summary>
        private void CloseBalloonCallback(object state)
        {
            if (IsDisposed) return;

            // switch to UI thread
            Action action = CloseBalloon;
            this.GetDispatcher().Invoke(action);
        }

        #endregion

        #region Process Incoming Mouse Events

        /// <summary>
        /// Processes mouse events, which are bubbled
        /// through the class' routed events, trigger
        /// certain actions (e.g. show a popup), or
        /// both.
        /// </summary>
        /// <param name="me">Event flag.</param>
        private void OnMouseEvent(MouseEvent me)
        {
            if (IsDisposed) return;

            switch (me)
            {
                case MouseEvent.MouseMove:
                    RaiseTrayMouseMoveEvent();
                    // immediately return - there's nothing left to evaluate
                    return;
                case MouseEvent.IconRightMouseDown:
                    RaiseTrayRightMouseDownEvent();
                    break;
                case MouseEvent.IconLeftMouseDown:
                    RaiseTrayLeftMouseDownEvent();
                    break;
                case MouseEvent.IconRightMouseUp:
                    RaiseTrayRightMouseUpEvent();
                    break;
                case MouseEvent.IconLeftMouseUp:
                    RaiseTrayLeftMouseUpEvent();
                    break;
                case MouseEvent.IconMiddleMouseDown:
                    RaiseTrayMiddleMouseDownEvent();
                    break;
                case MouseEvent.IconMiddleMouseUp:
                    RaiseTrayMiddleMouseUpEvent();
                    break;
                case MouseEvent.IconDoubleClick:
                    // cancel single click timer
                    singleClickTimer.Change(Timeout.Infinite, Timeout.Infinite);
                    // bubble event
                    RaiseTrayMouseDoubleClickEvent();
                    break;
                case MouseEvent.BalloonToolTipClicked:
                    RaiseTrayBalloonTipClickedEvent();
                    break;
                default:
                    throw new ArgumentOutOfRangeException(nameof(me), "Missing handler for mouse event flag: " + me);
            }


            // get mouse coordinates
            Point cursorPosition = new Point();
            if (messageSink.Version == NotifyIconVersion.Vista)
            {
                // physical cursor position is supported for Vista and above
                WinApi.GetPhysicalCursorPos(ref cursorPosition);
            }
            else
            {
                WinApi.GetCursorPos(ref cursorPosition);
            }

            cursorPosition = TrayInfo.GetDeviceCoordinates(cursorPosition);

            bool isLeftClickCommandInvoked = false;

            // show popup, if requested
            if (me.IsMatch(PopupActivation))
            {
                if (me == MouseEvent.IconLeftMouseUp)
                {
                    // show popup once we are sure it's not a double click
                    singleClickTimerAction = () =>
                    {
                        LeftClickCommand.ExecuteIfEnabled(LeftClickCommandParameter, LeftClickCommandTarget ?? this);
                        ShowTrayPopup(cursorPosition);
                    };
                    singleClickTimer.Change(DoubleClickWaitTime, Timeout.Infinite);
                    isLeftClickCommandInvoked = true;
                }
                else
                {
                    // show popup immediately
                    ShowTrayPopup(cursorPosition);
                }
            }


            // show context menu, if requested
            if (me.IsMatch(MenuActivation))
            {
                if (me == MouseEvent.IconLeftMouseUp)
                {
                    // show context menu once we are sure it's not a double click
                    singleClickTimerAction = () =>
                    {
                        LeftClickCommand.ExecuteIfEnabled(LeftClickCommandParameter, LeftClickCommandTarget ?? this);
                        ShowContextMenu(cursorPosition);
                    };
                    singleClickTimer.Change(DoubleClickWaitTime, Timeout.Infinite);
                    isLeftClickCommandInvoked = true;
                }
                else
                {
                    // show context menu immediately
                    ShowContextMenu(cursorPosition);
                }
            }

            // make sure the left click command is invoked on mouse clicks
            if (me == MouseEvent.IconLeftMouseUp && !isLeftClickCommandInvoked)
            {
                // show context menu once we are sure it's not a double click
                singleClickTimerAction =
                    () =>
                    {
                        LeftClickCommand.ExecuteIfEnabled(LeftClickCommandParameter, LeftClickCommandTarget ?? this);
                    };
                singleClickTimer.Change(DoubleClickWaitTime, Timeout.Infinite);
            }
        }

        #endregion

        #region ToolTips

        /// <summary>
        /// Displays a custom tooltip, if available. This method is only
        /// invoked for Windows Vista and above.
        /// </summary>
        /// <param name="visible">Whether to show or hide the tooltip.</param>
        private void OnToolTipChange(bool visible)
        {
            // if we don't have a tooltip, there's nothing to do here...
            if (TrayToolTipResolved == null) return;

            if (visible)
            {
                if (IsPopupOpen)
                {
                    // ignore if we are already displaying something down there
                    return;
                }

                var args = RaisePreviewTrayToolTipOpenEvent();
                if (args.Handled) return;

                TrayToolTipResolved.IsOpen = true;

                // raise attached event first
                if (TrayToolTip != null) RaiseToolTipOpenedEvent(TrayToolTip);

                // bubble routed event
                RaiseTrayToolTipOpenEvent();
            }
            else
            {
                var args = RaisePreviewTrayToolTipCloseEvent();
                if (args.Handled) return;

                // raise attached event first
                if (TrayToolTip != null) RaiseToolTipCloseEvent(TrayToolTip);

                TrayToolTipResolved.IsOpen = false;

                // bubble event
                RaiseTrayToolTipCloseEvent();
            }
        }


        /// <summary>
        /// Creates a <see cref="ToolTip"/> control that either
        /// wraps the currently set <see cref="TrayToolTip"/>
        /// control or the <see cref="ToolTipText"/> string.<br/>
        /// If <see cref="TrayToolTip"/> itself is already
        /// a <see cref="ToolTip"/> instance, it will be used directly.
        /// </summary>
        /// <remarks>We use a <see cref="ToolTip"/> rather than
        /// <see cref="Popup"/> because there was no way to prevent a
        /// popup from causing cyclic open/close commands if it was
        /// placed under the mouse. ToolTip internally uses a Popup of
        /// its own, but takes advance of Popup's internal <see cref="UIElement.IsHitTestVisible"/>
        /// property which prevents this issue.</remarks>
        private void CreateCustomToolTip()
        {
            // check if the item itself is a tooltip
            ToolTip tt = TrayToolTip as ToolTip;

            if (tt == null && TrayToolTip != null)
            {
                // create an invisible wrapper tooltip that hosts the UIElement
                tt = new ToolTip
                {
                    Placement = PlacementMode.Mouse,
                    // do *not* set the placement target, as it causes the popup to become hidden if the
                    // TaskbarIcon's parent is hidden, too. At runtime, the parent can be resolved through
                    // the ParentTaskbarIcon attached dependency property:
                    // PlacementTarget = this;

                    // make sure the tooltip is invisible
                    HasDropShadow = false,
                    BorderThickness = new Thickness(0),
                    Background = System.Windows.Media.Brushes.Transparent,
                    // setting the 
                    StaysOpen = true,
                    Content = TrayToolTip
                };
            }
            else if (tt == null && !string.IsNullOrEmpty(ToolTipText))
            {
                // create a simple tooltip for the ToolTipText string
                tt = new ToolTip
                {
                    Content = ToolTipText
                };
            }

            // the tooltip explicitly gets the DataContext of this instance.
            // If there is no DataContext, the TaskbarIcon assigns itself
            if (tt != null)
            {
                UpdateDataContext(tt, null, DataContext);
            }

            // store a reference to the used tooltip
            SetTrayToolTipResolved(tt);
        }


        /// <summary>
        /// Sets tooltip settings for the class depending on defined
        /// dependency properties and OS support.
        /// </summary>
        private void WriteToolTipSettings()
        {
            const IconDataMembers flags = IconDataMembers.Tip;
            iconData.ToolTipText = ToolTipText;

            if (messageSink.Version == NotifyIconVersion.Vista)
            {
                // we need to set a tooltip text to get tooltip events from the
                // taskbar icon
                if (string.IsNullOrEmpty(iconData.ToolTipText) && TrayToolTipResolved != null)
                {
                    // if we have not tooltip text but a custom tooltip, we
                    // need to set a dummy value (we're displaying the ToolTip control, not the string)
                    iconData.ToolTipText = "ToolTip";
                }
            }

            // update the tooltip text
            Util.WriteIconData(ref iconData, NotifyCommand.Modify, flags);
        }

        #endregion

        #region Custom Popup

        /// <summary>
        /// Creates a <see cref="ToolTip"/> control that either
        /// wraps the currently set <see cref="TrayToolTip"/>
        /// control or the <see cref="ToolTipText"/> string.<br/>
        /// If <see cref="TrayToolTip"/> itself is already
        /// a <see cref="ToolTip"/> instance, it will be used directly.
        /// </summary>
        /// <remarks>We use a <see cref="ToolTip"/> rather than
        /// <see cref="Popup"/> because there was no way to prevent a
        /// popup from causing cyclic open/close commands if it was
        /// placed under the mouse. ToolTip internally uses a Popup of
        /// its own, but takes advance of Popup's internal <see cref="UIElement.IsHitTestVisible"/>
        /// property which prevents this issue.</remarks>
        private void CreatePopup()
        {
            // check if the item itself is a popup
            Popup popup = TrayPopup as Popup;

            if (popup == null && TrayPopup != null)
            {
                // create an invisible popup that hosts the UIElement
                popup = new Popup
                {
                    AllowsTransparency = true,
                    // don't animate by default - developers can use attached events or override
                    PopupAnimation = PopupAnimation.None,
                    // the CreateRootPopup method outputs binding errors in the debug window because
                    // it tries to bind to "Popup-specific" properties in case they are provided by the child.
                    // We don't need that so just assign the control as the child.
                    Child = TrayPopup,
                    // do *not* set the placement target, as it causes the popup to become hidden if the
                    // TaskbarIcon's parent is hidden, too. At runtime, the parent can be resolved through
                    // the ParentTaskbarIcon attached dependency property:
                    // PlacementTarget = this;

                    Placement = PlacementMode.AbsolutePoint,
                    StaysOpen = false
                };
            }

            // the popup explicitly gets the DataContext of this instance.
            // If there is no DataContext, the TaskbarIcon assigns itself
            if (popup != null)
            {
                UpdateDataContext(popup, null, DataContext);
            }

            // store a reference to the used tooltip
            SetTrayPopupResolved(popup);
        }


        /// <summary>
        /// Displays the <see cref="TrayPopup"/> control if it was set.
        /// </summary>
        private void ShowTrayPopup(Point cursorPosition)
        {
            if (IsDisposed) return;

            // raise preview event no matter whether popup is currently set
            // or not (enables client to set it on demand)
            var args = RaisePreviewTrayPopupOpenEvent();
            if (args.Handled) return;

            if (TrayPopup == null)
            {
                return;
            }

            // use absolute position, but place the popup centered above the icon
            TrayPopupResolved.Placement = PlacementMode.AbsolutePoint;
            TrayPopupResolved.HorizontalOffset = cursorPosition.X;
            TrayPopupResolved.VerticalOffset = cursorPosition.Y;

            // open popup
            TrayPopupResolved.IsOpen = true;

            IntPtr handle = IntPtr.Zero;
            if (TrayPopupResolved.Child != null)
            {
                // try to get a handle on the popup itself (via its child)
                HwndSource source = (HwndSource)PresentationSource.FromVisual(TrayPopupResolved.Child);
                if (source != null) handle = source.Handle;
            }

            // if we don't have a handle for the popup, fall back to the message sink
            if (handle == IntPtr.Zero) handle = messageSink.MessageWindowHandle;

            // activate either popup or message sink to track deactivation.
            // otherwise, the popup does not close if the user clicks somewhere else
            WinApi.SetForegroundWindow(handle);

            // raise attached event - item should never be null unless developers
            // changed the CustomPopup directly...
            if (TrayPopup != null) RaisePopupOpenedEvent(TrayPopup);

            // bubble routed event
            RaiseTrayPopupOpenEvent();
        }

        #endregion

        #region Context Menu

        /// <summary>
        /// Displays the <see cref="ContextMenu"/> if it was set.
        /// </summary>
        private void ShowContextMenu(Point cursorPosition)
        {
            if (IsDisposed) return;

            // raise preview event no matter whether context menu is currently set
            // or not (enables client to set it on demand)
            var args = RaisePreviewTrayContextMenuOpenEvent();
            if (args.Handled) return;

            if (ContextMenu == null)
            {
                return;
            }

            // use absolute positioning. We need to set the coordinates, or a delayed opening
            // (e.g. when left-clicked) opens the context menu at the wrong place if the mouse
            // is moved!
            ContextMenu.Placement = PlacementMode.AbsolutePoint;
            ContextMenu.HorizontalOffset = cursorPosition.X;
            ContextMenu.VerticalOffset = cursorPosition.Y;
            ContextMenu.IsOpen = true;

            IntPtr handle = IntPtr.Zero;

            // try to get a handle on the context itself
            HwndSource source = (HwndSource)PresentationSource.FromVisual(ContextMenu);
            if (source != null)
            {
                handle = source.Handle;
            }

            // if we don't have a handle for the popup, fall back to the message sink
            if (handle == IntPtr.Zero) handle = messageSink.MessageWindowHandle;

            // activate the context menu or the message window to track deactivation - otherwise, the context menu
            // does not close if the user clicks somewhere else. With the message window
            // fallback, the context menu can't receive keyboard events - should not happen though
            WinApi.SetForegroundWindow(handle);

            // bubble event
            RaiseTrayContextMenuOpenEvent();
        }

        #endregion

        #region Balloon Tips

        /// <summary>
        /// Bubbles events if a balloon ToolTip was displayed
        /// or removed.
        /// </summary>
        /// <param name="visible">Whether the ToolTip was just displayed
        /// or removed.</param>
        private void OnBalloonToolTipChanged(bool visible)
        {
            if (visible)
            {
                RaiseTrayBalloonTipShownEvent();
            }
            else
            {
                RaiseTrayBalloonTipClosedEvent();
            }
        }

        /// <summary>
        /// Displays a balloon tip with the specified title,
        /// text, and icon in the taskbar for the specified time period.
        /// </summary>
        /// <param name="title">The title to display on the balloon tip.</param>
        /// <param name="message">The text to display on the balloon tip.</param>
        /// <param name="symbol">A symbol that indicates the severity.</param>
        public void ShowBalloonTip(string title, string message, BalloonIcon symbol)
        {
            lock (lockObject)
            {
                ShowBalloonTip(title, message, symbol.GetBalloonFlag(), IntPtr.Zero);
            }
        }

        /// <summary>
        /// Displays a balloon tip with the specified title,
        /// text, and a custom icon in the taskbar for the specified time period.
        /// </summary>
        /// <param name="title">The title to display on the balloon tip.</param>
        /// <param name="message">The text to display on the balloon tip.</param>
        /// <param name="customIcon">A custom icon.</param>
        /// <param name="largeIcon">True to allow large icons (Windows Vista and later).</param>
        /// <exception cref="ArgumentNullException">If <paramref name="customIcon"/>
        /// is a null reference.</exception>
        public void ShowBalloonTip(string title, string message, Icon customIcon, bool largeIcon = false)
        {
            if (customIcon == null) throw new ArgumentNullException(nameof(customIcon));

            lock (lockObject)
            {
                var flags = BalloonFlags.User;

                if (largeIcon)
                {
                    // ReSharper disable once BitwiseOperatorOnEnumWithoutFlags
                    flags |= BalloonFlags.LargeIcon;
                }

                ShowBalloonTip(title, message, flags, customIcon.Handle);
            }
        }


        /// <summary>
        /// Invokes <see cref="WinApi.Shell_NotifyIcon"/> in order to display
        /// a given balloon ToolTip.
        /// </summary>
        /// <param name="title">The title to display on the balloon tip.</param>
        /// <param name="message">The text to display on the balloon tip.</param>
        /// <param name="flags">Indicates what icon to use.</param>
        /// <param name="balloonIconHandle">A handle to a custom icon, if any, or
        /// <see cref="IntPtr.Zero"/>.</param>
        private void ShowBalloonTip(string title, string message, BalloonFlags flags, IntPtr balloonIconHandle)
        {
            EnsureNotDisposed();

            iconData.BalloonText = message ?? string.Empty;
            iconData.BalloonTitle = title ?? string.Empty;

            iconData.BalloonFlags = flags;
            iconData.CustomBalloonIconHandle = balloonIconHandle;
            Util.WriteIconData(ref iconData, NotifyCommand.Modify, IconDataMembers.Info | IconDataMembers.Icon);
        }


        /// <summary>
        /// Hides a balloon ToolTip, if any is displayed.
        /// </summary>
        public void HideBalloonTip()
        {
            EnsureNotDisposed();

            // reset balloon by just setting the info to an empty string
            iconData.BalloonText = iconData.BalloonTitle = string.Empty;
            Util.WriteIconData(ref iconData, NotifyCommand.Modify, IconDataMembers.Info);
        }

        #endregion

        #region Single Click Timer event

        /// <summary>
        /// Performs a delayed action if the user requested an action
        /// based on a single click of the left mouse.<br/>
        /// This method is invoked by the <see cref="singleClickTimer"/>.
        /// </summary>
        private void DoSingleClickAction(object state)
        {
            if (IsDisposed) return;

            // run action
            Action action = singleClickTimerAction;
            if (action != null)
            {
                // cleanup action
                singleClickTimerAction = null;

                // switch to UI thread
                this.GetDispatcher().Invoke(action);
            }
        }

        #endregion

        #region Set Version (API)

        /// <summary>
        /// Sets the version flag for the <see cref="iconData"/>.
        /// </summary>
        private void SetVersion()
        {
            iconData.VersionOrTimeout = (uint)NotifyIconVersion.Vista;
            bool status = WinApi.Shell_NotifyIcon(NotifyCommand.SetVersion, ref iconData);

            if (!status)
            {
                iconData.VersionOrTimeout = (uint)NotifyIconVersion.Win2000;
                status = Util.WriteIconData(ref iconData, NotifyCommand.SetVersion);
            }

            if (!status)
            {
                iconData.VersionOrTimeout = (uint)NotifyIconVersion.Win95;
                status = Util.WriteIconData(ref iconData, NotifyCommand.SetVersion);
            }

            if (!status)
            {
                Debug.Fail("Could not set version");
            }
        }

        #endregion

        #region Create / Remove Taskbar Icon

        /// <summary>
        /// Recreates the taskbar icon if the whole taskbar was
        /// recreated (e.g. because Explorer was shut down).
        /// </summary>
        private void OnTaskbarCreated()
        {
            IsTaskbarIconCreated = false;
            CreateTaskbarIcon();
        }


        /// <summary>
        /// Creates the taskbar icon. This message is invoked during initialization,
        /// if the taskbar is restarted, and whenever the icon is displayed.
        /// </summary>
        private void CreateTaskbarIcon()
        {
            lock (lockObject)
            {
                if (IsTaskbarIconCreated)
                {
                    return;
                }

                const IconDataMembers members = IconDataMembers.Message
                                                | IconDataMembers.Icon
                                                | IconDataMembers.Tip;

                //write initial configuration
                var status = Util.WriteIconData(ref iconData, NotifyCommand.Add, members);
                if (!status)
                {
                    // couldn't create the icon - we can assume this is because explorer is not running (yet!)
                    // -> try a bit later again rather than throwing an exception. Typically, if the windows
                    // shell is being loaded later, this method is being re-invoked from OnTaskbarCreated
                    // (we could also retry after a delay, but that's currently YAGNI)
                    return;
                }

                //set to most recent version
                SetVersion();
                messageSink.Version = (NotifyIconVersion)iconData.VersionOrTimeout;

                IsTaskbarIconCreated = true;
            }
        }

        /// <summary>
        /// Closes the taskbar icon if required.
        /// </summary>
        private void RemoveTaskbarIcon()
        {
            lock (lockObject)
            {
                // make sure we didn't schedule a creation

                if (!IsTaskbarIconCreated)
                {
                    return;
                }

                Util.WriteIconData(ref iconData, NotifyCommand.Delete, IconDataMembers.Message);
                IsTaskbarIconCreated = false;
            }
        }

        #endregion



        #region Dispose / Exit

        /// <summary>
        /// Set to true as soon as <c>Dispose</c> has been invoked.
        /// </summary>
        public bool IsDisposed { get; private set; }


        /// <summary>
        /// Checks if the object has been disposed and
        /// raises a <see cref="ObjectDisposedException"/> in case
        /// the <see cref="IsDisposed"/> flag is true.
        /// </summary>
        private void EnsureNotDisposed()
        {
            if (IsDisposed) throw new ObjectDisposedException(Name ?? GetType().FullName);
        }


        /// <summary>
        /// Disposes the class if the application exits.
        /// </summary>
        private void OnExit(object sender, EventArgs e)
        {
            Dispose();
        }


        /// <summary>
        /// This destructor will run only if the <see cref="Dispose()"/>
        /// method does not get called. This gives this base class the
        /// opportunity to finalize.
        /// <para>
        /// Important: Do not provide destructor in types derived from this class.
        /// </para>
        /// </summary>
        ~TaskbarIcon()
        {
            Dispose(false);
        }


        /// <summary>
        /// Disposes the object.
        /// </summary>
        /// <remarks>This method is not virtual by design. Derived classes
        /// should override <see cref="Dispose(bool)"/>.
        /// </remarks>
        public void Dispose()
        {
            Dispose(true);

            // This object will be cleaned up by the Dispose method.
            // Therefore, you should call GC.SuppressFinalize to
            // take this object off the finalization queue 
            // and prevent finalization code for this object
            // from executing a second time.
            GC.SuppressFinalize(this);
        }


        /// <summary>
        /// Closes the tray and releases all resources.
        /// </summary>
        /// <summary>
        /// <c>Dispose(bool disposing)</c> executes in two distinct scenarios.
        /// If disposing equals <c>true</c>, the method has been called directly
        /// or indirectly by a user's code. Managed and unmanaged resources
        /// can be disposed.
        /// </summary>
        /// <param name="disposing">If disposing equals <c>false</c>, the method
        /// has been called by the runtime from inside the finalizer and you
        /// should not reference other objects. Only unmanaged resources can
        /// be disposed.</param>
        /// <remarks>Check the <see cref="IsDisposed"/> property to determine whether
        /// the method has already been called.</remarks>
        private void Dispose(bool disposing)
        {
            // don't do anything if the component is already disposed
            if (IsDisposed || !disposing) return;

            lock (lockObject)
            {
                IsDisposed = true;

                // de-register application event listener
                if (Application.Current != null)
                {
                    Application.Current.Exit -= OnExit;
                }

                // stop timers
                singleClickTimer.Dispose();
                balloonCloseTimer.Dispose();

                // dispose message sink
                messageSink.Dispose();

                // remove icon
                RemoveTaskbarIcon();
            }
        }

        #endregion
    }
}