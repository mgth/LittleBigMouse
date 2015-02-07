using System;
using System.ComponentModel;
using System.Windows.Forms;
using MouseKeyboardActivityMonitor.WinApi;

namespace MouseKeyboardActivityMonitor.Controls
{
    /// <summary>
    /// This component monitors Application or Global input, depending on 
	/// <see cref="MouseKeyEventProvider.Enabled"/> and provides appropriate
	/// events.
	/// </summary>
    public class MouseKeyEventProvider : Component
    {
        private readonly KeyboardHookListener m_KeyboardHookManager;
        private readonly MouseHookListener m_MouseHookManager;

        /// <summary>
        /// Initializes a new instance of <see cref="MouseKeyEventProvider"/>
        /// </summary>
        public MouseKeyEventProvider()
        {
            m_KeyboardHookManager = new KeyboardHookListener(new GlobalHooker());
            m_MouseHookManager = new MouseHookListener(new GlobalHooker());
        }

        /// <summary>
        /// Gets or Sets the enabled status of the component.
        /// </summary>
        /// <value>
        /// True - The component is presently activated and will fire events.
        /// <para>
        /// False - The component is not active and will not fire events.
        /// </para>
        /// </value>
        public bool Enabled
        {
            get
            {
                return DesignMode
                           ? DesignTimeEnabled
                           : RunTimeEnabled;
            }
            set
            {
                if (DesignMode)
                {
                    DesignTimeEnabled = value;
                }
                else
                {
                    RunTimeEnabled = value;
                }
            }
        }

        private bool DesignTimeEnabled
        {
            get; set;
        }

        private bool RunTimeEnabled
        {
            get
            {
                return m_MouseHookManager.Enabled && m_KeyboardHookManager.Enabled;
            }
            set
            {
                m_MouseHookManager.Enabled = value;
                m_KeyboardHookManager.Enabled = value;
            }
        }


        ///<summary>
        /// Indicates which hooks to listen to application or global.
        ///</summary>
        public HookType HookType
        {
            get
            {
                return m_MouseHookManager.IsGlobal
                           ? HookType.Global
                           : HookType.Application;
            }
            set
            {
                Hooker hooker;
                switch (value)
                {
                    case HookType.Global:
                        hooker = new GlobalHooker();
                        break;

                    case HookType.Application:
                        hooker = new AppHooker();
                        break;

                    default:
                        return;
                }

                m_MouseHookManager.Replace(hooker);
                m_KeyboardHookManager.Replace(hooker);
            }
        }

        /// <summary>
        /// This component raises events. The value is always true.
        /// </summary>
        protected override bool CanRaiseEvents
        {
            get
            {
                return true;
            }
        }

        /// <summary>
        /// 
        /// </summary>
        /// <param name="inputEvent"></param>
        /// <param name="e"></param>
        /// <remarks>
        /// Modified from http://stackoverflow.com/questions/1698889/raise-events-in-net-on-the-main-ui-thread
        /// </remarks>
        private void RaiseEventOnUIThread(Delegate inputEvent, EventArgs e)
        {
            object sender = this;
            foreach (Delegate d in inputEvent.GetInvocationList())
            {
                ISynchronizeInvoke syncer = d.Target as ISynchronizeInvoke;

                if (syncer == null)
                {
                    d.DynamicInvoke(new[] { sender, e });
                }
                else
                {
                    // I don't know if ASyncronous is really the way to go.
                    //  If the programmer wants to suppress input,
                    //  will asyncronous make that happen consistently?

                    //syncer.EndInvoke(syncer.BeginInvoke(inputEvent, new[] { sender, e }));
                    syncer.Invoke(inputEvent, new[] { sender, e });
                }
            }
        }

        //################################################################
        #region Mouse events

        private event MouseEventHandler m_MouseMove;
        /// <summary>
        /// Activated when the user moves the mouse. 
        /// </summary>
        public event MouseEventHandler MouseMove
        {
            add
            {
                if (m_MouseMove == null)
                {
                    m_MouseHookManager.MouseMove += HookManager_MouseMove;
                }
                m_MouseMove += value;
            }

            remove
            {
                m_MouseMove -= value;
                if (m_MouseMove == null)
                {
                    m_MouseHookManager.MouseMove -= HookManager_MouseMove;
                }
            }
        }

        void HookManager_MouseMove(object sender, MouseEventArgs e)
        {
            if (m_MouseMove != null)
            {
                RaiseEventOnUIThread(m_MouseMove, e);
            }
        }

        private event MouseEventHandler m_MouseClick;
        /// <summary>
        /// Activated upon a single click of the mouse.
        /// </summary>
        public event MouseEventHandler MouseClick
        {
            add
            {
                if (m_MouseClick == null)
                {
                    m_MouseHookManager.MouseClick += HookManager_MouseClick;
                }
                m_MouseClick += value;
            }

            remove
            {
                m_MouseClick -= value;
                if (m_MouseClick == null)
                {
                    m_MouseHookManager.MouseClick -= HookManager_MouseClick;
                }
            }
        }

        void HookManager_MouseClick(object sender, MouseEventArgs e)
        {
            if (m_MouseClick != null)
            {
                RaiseEventOnUIThread(m_MouseClick, e);
            }
        }

        private event MouseEventHandler m_MouseDown;

        /// <summary>
        /// Activated when the user presses a mouse button.
        /// </summary>
        public event MouseEventHandler MouseDown
        {
            add
            {
                if (m_MouseDown == null)
                {
                    m_MouseHookManager.MouseDown += HookManager_MouseDown;
                }
                m_MouseDown += value;
            }

            remove
            {
                m_MouseDown -= value;
                if (m_MouseDown == null)
                {
                    m_MouseHookManager.MouseDown -= HookManager_MouseDown;
                }
            }
        }

        void HookManager_MouseDown(object sender, MouseEventArgs e)
        {
            if (m_MouseDown != null)
            {
                RaiseEventOnUIThread(m_MouseDown, e);
            }
        }


        private event MouseEventHandler m_MouseUp;

        /// <summary>
        /// Activated when the user releases a mouse button.
        /// </summary>
        public event MouseEventHandler MouseUp
        {
            add
            {
                if (m_MouseUp == null)
                {
                    m_MouseHookManager.MouseUp += HookManager_MouseUp;
                }
                m_MouseUp += value;
            }

            remove
            {
                m_MouseUp -= value;
                if (m_MouseUp == null)
                {
                    m_MouseHookManager.MouseUp -= HookManager_MouseUp;
                }
            }
        }

        void HookManager_MouseUp(object sender, MouseEventArgs e)
        {
            if (m_MouseUp != null)
            {
                RaiseEventOnUIThread(m_MouseUp, e);
            }
        }

        private event MouseEventHandler m_MouseDoubleClick;

        /// <summary>
        /// Activated when the user double-clicks with the mouse.
        /// </summary>
        public event MouseEventHandler MouseDoubleClick
        {
            add
            {
                if (m_MouseDoubleClick == null)
                {
                    m_MouseHookManager.MouseDoubleClick += HookManager_MouseDoubleClick;
                }
                m_MouseDoubleClick += value;
            }

            remove
            {
                m_MouseDoubleClick -= value;
                if (m_MouseDoubleClick == null)
                {
                    m_MouseHookManager.MouseDoubleClick -= HookManager_MouseDoubleClick;
                }
            }
        }

        void HookManager_MouseDoubleClick(object sender, MouseEventArgs e)
        {
            if (m_MouseDoubleClick != null)
            {
                RaiseEventOnUIThread(m_MouseDoubleClick, e);
            }
        }


        private event EventHandler<MouseEventExtArgs> m_MouseMoveExt;

        /// <summary>
        /// Activated when the user moves the mouse. 
        /// </summary>
        /// <remarks>
        /// This event provides extended arguments of type <see cref="MouseEventArgs"/> enabling you to 
        /// supress further processing of mouse movement in other applications.
        /// </remarks>
        public event EventHandler<MouseEventExtArgs> MouseMoveExt
        {
            add
            {
                if (m_MouseMoveExt == null)
                {
                    m_MouseHookManager.MouseMoveExt += HookManager_MouseMoveExt;
                }
                m_MouseMoveExt += value;
            }

            remove
            {
                m_MouseMoveExt -= value;
                if (m_MouseMoveExt == null)
                {
                    m_MouseHookManager.MouseMoveExt -= HookManager_MouseMoveExt;
                }
            }
        }

        void HookManager_MouseMoveExt(object sender, MouseEventExtArgs e)
        {
            if (m_MouseMoveExt != null)
            {
                RaiseEventOnUIThread(m_MouseMoveExt, e);
            }
        }

        private event EventHandler<MouseEventExtArgs> m_MouseClickExt;
        /// <summary>
        /// Activated upon a single click of the mouse.
        /// </summary>
        /// <remarks>
        /// This event provides extended arguments of type <see cref="MouseEventArgs"/> enabling you to 
        /// supress further processing of mouse click in other applications.
        /// </remarks>
        public event EventHandler<MouseEventExtArgs> MouseClickExt
        {
            add
            {
                // Disable warning that MouseClickExt is obsolete
#pragma warning disable 618
                if (m_MouseClickExt == null)
                {
                    m_MouseHookManager.MouseClickExt += HookManager_MouseClickExt;
                }
                m_MouseClickExt += value;
            }

            remove
            {
                m_MouseClickExt -= value;
                if (m_MouseClickExt == null)
                {
                    m_MouseHookManager.MouseClickExt -= HookManager_MouseClickExt;
                }
#pragma warning restore 618
            }
        }

        void HookManager_MouseClickExt(object sender, MouseEventExtArgs e)
        {
            if (m_MouseClickExt != null)
            {
                RaiseEventOnUIThread(m_MouseClickExt, e);
            }
        }

        private event EventHandler<MouseEventExtArgs> m_MouseDownExt;

        /// <summary>
        /// Activated when the user presses a mouse button.
        /// </summary>
        /// <remarks>
        /// This event provides extended arguments of type <see cref="MouseEventArgs"/> enabling you to 
        /// supress further processing of mouse down in other applications.
        /// </remarks>
        public event EventHandler<MouseEventExtArgs> MouseDownExt
        {
            add
            {
                if (m_MouseDownExt == null)
                {
                    m_MouseHookManager.MouseDownExt += HookManager_MouseDownExt;
                }
                m_MouseDownExt += value;
            }

            remove
            {
                m_MouseDownExt -= value;
                if (m_MouseDownExt == null)
                {
                    m_MouseHookManager.MouseDownExt -= HookManager_MouseDownExt;
                }
            }
        }

        void HookManager_MouseDownExt(object sender, MouseEventExtArgs e)
        {
            if (m_MouseDownExt != null)
            {
                RaiseEventOnUIThread(m_MouseDownExt, e);
            }
        }
        
        private event EventHandler<MouseEventArgs> m_MouseWheel;
        /// <summary>
        /// Activated upon mouse scrolling.
        /// </summary>
        public event EventHandler<MouseEventArgs> MouseWheel
        {
            add
            {
                if (m_MouseWheel == null)
                {
                    m_MouseHookManager.MouseWheel += HookManager_MouseWheel;
                }
                m_MouseWheel += value;
            }

            remove
            {
                m_MouseWheel -= value;
                if (m_MouseWheel == null)
                {
                    m_MouseHookManager.MouseWheel -= HookManager_MouseWheel;
                }
            }
        }

        void HookManager_MouseWheel(object sender, MouseEventArgs e)
        {
            if (m_MouseWheel != null)
            {
                RaiseEventOnUIThread(m_MouseWheel, e);
            }
        }


        #endregion

        //################################################################
        #region Keyboard events

        private event KeyPressEventHandler m_KeyPress;

        /// <summary>
        /// Activated when the user presses a key.
        /// </summary>
        /// <remarks>
        /// Key events occur in the following order: 
        /// <list type="number">
        /// <item>KeyDown</item>
        /// <item>KeyPress</item>
        /// <item>KeyUp</item>
        /// </list>
        ///The KeyPress event is not raised by noncharacter keys; however, the noncharacter keys do raise the KeyDown and KeyUp events. 
        ///Use the KeyChar property to sample keystrokes at run time and to consume or modify a subset of common keystrokes. 
        ///To handle keyboard events only in your application and not enable other applications to receive keyboard events, 
        /// set the KeyPressEventArgs.Handled property in your form's KeyPress event-handling method to <b>true</b>. 
        /// </remarks>
        public event KeyPressEventHandler KeyPress
        {
            add
            {
                if (m_KeyPress == null)
                {
                    m_KeyboardHookManager.KeyPress += HookManager_KeyPress;
                }
                m_KeyPress += value;
            }
            remove
            {
                m_KeyPress -= value;
                if (m_KeyPress == null)
                {
                    m_KeyboardHookManager.KeyPress -= HookManager_KeyPress;
                }
            }
        }

        void HookManager_KeyPress(object sender, KeyPressEventArgs e)
        {
            if (m_KeyPress != null)
            {
                RaiseEventOnUIThread(m_KeyPress, e);
            }
        }

        private event KeyEventHandler m_KeyUp;

        /// <summary>
        /// Activated upon the release of a key.
        /// </summary>
        public event KeyEventHandler KeyUp
        {
            add
            {
                if (m_KeyUp == null)
                {
                    m_KeyboardHookManager.KeyUp += HookManager_KeyUp;
                }
                m_KeyUp += value;
            }
            remove
            {
                m_KeyUp -= value;
                if (m_KeyUp == null)
                {
                    m_KeyboardHookManager.KeyUp -= HookManager_KeyUp;
                }
            }
        }

        private void HookManager_KeyUp(object sender, KeyEventArgs e)
        {
            if (m_KeyUp != null)
            {
                RaiseEventOnUIThread(m_KeyUp, e);
            }
        }

        private event KeyEventHandler m_KeyDown;

        /// <summary>
        /// Activated when a key is pushed.
        /// </summary>
        public event KeyEventHandler KeyDown
        {
            add
            {
                if (m_KeyDown == null)
                {
                    m_KeyboardHookManager.KeyDown += HookManager_KeyDown;
                }
                m_KeyDown += value;
            }
            remove
            {
                m_KeyDown -= value;
                if (m_KeyDown == null)
                {
                    m_KeyboardHookManager.KeyDown -= HookManager_KeyDown;
                }
            }
        }

        private void HookManager_KeyDown(object sender, KeyEventArgs e)
        {
            RaiseEventOnUIThread(m_KeyDown, e);
        }

        #endregion


    }
}
