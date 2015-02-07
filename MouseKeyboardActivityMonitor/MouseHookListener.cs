using System;
using System.Windows.Forms;
using MouseKeyboardActivityMonitor.WinApi;

namespace MouseKeyboardActivityMonitor
{
	/// <summary>
	/// This class monitors all mouse activities and provides appropriate events.
	/// </summary>
	public class MouseHookListener : BaseHookListener
	{

		private Point m_PreviousPosition;
        private int m_PreviousClickedTime;
        private Point m_PreviousClickedPosition;
		private MouseButtons m_PreviousClicked;
		private MouseButtons m_DownButtonsWaitingForMouseUp;
		private MouseButtons m_SuppressButtonUpFlags;
        private int m_SystemDoubleClickTime;

		/// <summary>
		/// Initializes a new instance of <see cref="MouseHookListener"/>.
		/// </summary>
		/// <param name="hooker">Depending on this parameter the listener hooks either application or global mouse events.</param>
		/// <remarks>
        /// Hooks are not active after installation. You need to use either <see cref="BaseHookListener.Enabled"/> property or call <see cref="BaseHookListener.Start"/> method.
        /// </remarks>
		public MouseHookListener(Hooker hooker)
			: base(hooker)
		{
			m_PreviousPosition = new Point(-1, -1);
            m_PreviousClickedTime = 0;
			m_DownButtonsWaitingForMouseUp = MouseButtons.None;
            m_SuppressButtonUpFlags = MouseButtons.None;
			m_PreviousClicked = MouseButtons.None;
            m_SystemDoubleClickTime = MouseNativeMethods.GetDoubleClickTime();
		}

        //##################################################################
        #region ProcessCallback and related subroutines

        /// <summary>
        /// This method processes the data from the hook and initiates event firing.
        /// </summary>
        /// <param name="wParam">The first Windows Messages parameter.</param>
        /// <param name="lParam">The second Windows Messages parameter.</param>
        /// <returns>
        /// True - The hook will be passed along to other applications.
        /// <para>
        /// False - The hook will not be given to other applications, effectively blocking input.
        /// </para>
        /// </returns>
        protected override bool ProcessCallback(int wParam, IntPtr lParam)
        {
            MouseEventExtArgs e = MouseEventExtArgs.FromRawData(wParam, lParam, IsGlobal);

            if (e.IsMouseKeyDown)
            {
                ProcessMouseDown(ref e);
            }

            if (e.Clicks == 1 && e.IsMouseKeyUp && !e.Handled)
            {
                ProcessMouseClick(ref e);
            }

            if (e.Clicks == 2 && !e.Handled)
            {
                InvokeMouseEventHandler(MouseDoubleClick, e);
            }

            if (e.IsMouseKeyUp)
            {
                ProcessMouseUp(ref e);
            }

            if (e.WheelScrolled)
            {
                InvokeMouseEventHandler(MouseWheel, e);
            }

            if (HasMoved(e.Point))
            {
                ProcessMouseMove(ref e);
            }

            return !e.Handled;
        }

        private void ProcessMouseDown(ref MouseEventExtArgs e)
        {
            if (IsGlobal)
            {
                ProcessPossibleDoubleClick(ref e);
            }
            else
            {
                // These are only used for global. No need for them in AppHooks
                m_DownButtonsWaitingForMouseUp = MouseButtons.None;
                m_PreviousClicked = MouseButtons.None;
                m_PreviousClickedTime = 0;
            } 
            

            InvokeMouseEventHandler(MouseDown, e);
            InvokeMouseEventHandlerExt(MouseDownExt, e);
            if (e.Handled)
            {
                SetSupressButtonUpFlag(e.Button);
                e.Handled = true;
            }
        }

        private void ProcessPossibleDoubleClick(ref MouseEventExtArgs e)
        {
            if (IsDoubleClick(e.Button, e.Timestamp, e.Point))
            {
                e = e.ToDoubleClickEventArgs();
                m_DownButtonsWaitingForMouseUp = MouseButtons.None;
                m_PreviousClicked = MouseButtons.None;
                m_PreviousClickedTime = 0;
            }
            else
            {
                m_DownButtonsWaitingForMouseUp |= e.Button;
                m_PreviousClickedTime = e.Timestamp;
            }
        }

        private void ProcessMouseClick(ref MouseEventExtArgs e)
        {
            if ((m_DownButtonsWaitingForMouseUp & e.Button) != MouseButtons.None)
            {
                m_PreviousClicked = e.Button;
                m_PreviousClickedPosition = e.Point;
                m_DownButtonsWaitingForMouseUp = MouseButtons.None;
                InvokeMouseEventHandler(MouseClick, e);
                InvokeMouseEventHandlerExt(MouseClickExt, e);
            }
        }

        private void ProcessMouseUp(ref MouseEventExtArgs e)
        {
            if (!HasSupressButtonUpFlag(e.Button))
            {
                InvokeMouseEventHandler(MouseUp, e);
            }
            else
            {
                RemoveSupressButtonUpFlag(e.Button);
                e.Handled = true;
            }
        }

        private void ProcessMouseMove(ref MouseEventExtArgs e)
        {
            m_PreviousPosition = e.Point;

            InvokeMouseEventHandler(MouseMove, e);
            InvokeMouseEventHandlerExt(MouseMoveExt, e);
        }

        #endregion

        private void RemoveSupressButtonUpFlag(MouseButtons button)
		{
			m_SuppressButtonUpFlags = m_SuppressButtonUpFlags ^ button;
		}

		private bool HasSupressButtonUpFlag(MouseButtons button)
		{
			return (m_SuppressButtonUpFlags & button) != 0;
		}

		private void SetSupressButtonUpFlag(MouseButtons button)
		{
			m_SuppressButtonUpFlags = m_SuppressButtonUpFlags | button;
		}

		/// <summary>
		/// Returns the correct hook id to be used for <see cref="HookNativeMethods.SetWindowsHookEx"/> call.
		/// </summary>
		/// <returns>WH_MOUSE (0x07) or WH_MOUSE_LL (0x14) constant.</returns>
		protected override int GetHookId()
		{
			return IsGlobal ?
				GlobalHooker.WH_MOUSE_LL :
				AppHooker.WH_MOUSE;
		}

		private bool HasMoved(Point actualPoint)
		{
			return m_PreviousPosition != actualPoint;
		}

		private bool IsDoubleClick(MouseButtons button, int timestamp, Point pos)
		{
            return
                button == m_PreviousClicked &&
                pos == m_PreviousClickedPosition && // Click-move-click exception, see Patch 11222
                timestamp - m_PreviousClickedTime <= m_SystemDoubleClickTime; // Mouse.GetDoubleClickTime();
		}

		private void InvokeMouseEventHandler(MouseEventHandler handler, MouseEventArgs e)
		{
			if (handler != null)
			{
				handler(this, e);
			}
		}


		private void InvokeMouseEventHandlerExt(EventHandler<MouseEventExtArgs> handler, MouseEventExtArgs e)
		{
			if (handler != null)
			{
				handler(this, e);
			}
		}

		/// <summary>
		/// Occurs when the mouse pointer is moved.
		/// </summary>
		public event MouseEventHandler MouseMove;

		/// <summary>
		/// Occurs when the mouse pointer is moved.
		/// </summary>
		/// <remarks>
		/// This event provides extended arguments of type <see cref = "MouseEventArgs" /> enabling you to 
		/// suppress further processing of mouse movement in other applications.
		/// </remarks>
		public event EventHandler<MouseEventExtArgs> MouseMoveExt;

		/// <summary>
		/// Occurs when a click was performed by the mouse.
		/// </summary>
		public event MouseEventHandler MouseClick;

		/// <summary>
		/// Occurs when a click was performed by the mouse.
		/// </summary>
		/// <remarks>
		/// This event provides extended arguments of type <see cref = "MouseEventArgs" /> enabling you to 
		/// suppress further processing of mouse click in other applications.
		/// </remarks>
		[Obsolete("To suppress mouse clicks use MouseDownExt event instead.")]
		public event EventHandler<MouseEventExtArgs> MouseClickExt;

		/// <summary>
		/// Occurs when the mouse a mouse button is pressed.
		/// </summary>
		public event MouseEventHandler MouseDown;

		/// <summary>
		/// Occurs when the mouse a mouse button is pressed.
		/// </summary>
		/// <remarks>
		/// This event provides extended arguments of type <see cref = "MouseEventArgs" /> enabling you to 
		/// suppress further processing of mouse click in other applications.
		/// </remarks>
		public event EventHandler<MouseEventExtArgs> MouseDownExt;

		/// <summary>
		/// Occurs when a mouse button is released.
		/// </summary>
		public event MouseEventHandler MouseUp;

		/// <summary>
		/// Occurs when the mouse wheel moves.
		/// </summary>
		public event MouseEventHandler MouseWheel;

		/// <summary>
		/// Occurs when a mouse button is double-clicked.
		/// </summary>
		public event MouseEventHandler MouseDoubleClick;

	    /// <summary>
	    /// Method to be used from <see cref="Dispose"/> and Finalizer.
	    /// Override this method to release subclass specific references.
	    /// </summary>
	    protected override void Dispose(bool isDisposing)
		{
            if (isDisposing)
            {
                MouseClick = null;
                MouseClickExt = null;
                MouseDown = null;
                MouseDownExt = null;
                MouseMove = null;
                MouseMoveExt = null;
                MouseUp = null;
                MouseWheel = null;
                MouseDoubleClick = null;
            }
		    base.Dispose(isDisposing);
		}
	}
}