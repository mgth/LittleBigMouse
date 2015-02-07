using System;
using MouseKeyboardActivityMonitor.WinApi;

namespace MouseKeyboardActivityMonitor
{
    /// <summary>
    /// Base class used to implement mouse or keyboard hook listeners.
    /// It provides base methods to subscribe and unsubscribe to hooks.
    /// Common processing, error handling and cleanup logic.
    /// </summary>
    public abstract class BaseHookListener : IDisposable
    {
        private Hooker m_Hooker;

        /// <summary>
        /// Base constructor of <see cref="BaseHookListener"/>
        /// </summary>
        /// <param name="hooker">Depending on this parameter the listener hooks either application or global keyboard events.</param>
        /// <remarks>
        /// Hooks are not active after instantiation. You need to use either <see cref="BaseHookListener.Enabled"/> property or call <see cref="BaseHookListener.Start"/> method.
        /// </remarks>
        protected BaseHookListener(Hooker hooker)
        {
            if (hooker == null)
            {
                throw new ArgumentNullException("hooker");
            }
            m_Hooker = hooker;
        }

        /// <summary>
        /// Stores the handle to the Keyboard or Mouse hook procedure.
        /// </summary>
        protected int HookHandle { get; set; }

        /// <summary>
        /// Keeps the reference to prevent garbage collection of delegate. See: CallbackOnCollectedDelegate http://msdn.microsoft.com/en-us/library/43yky316(v=VS.100).aspx
        /// </summary>
        protected HookCallback HookCallbackReferenceKeeper { get; set; }

        internal bool IsGlobal
        {
            get
            {
                return m_Hooker.IsGlobal;
            }
        }

        /// <summary>
        /// Override this method to modify logic of firing events.
        /// </summary>
        protected abstract bool ProcessCallback(int wParam, IntPtr lParam);

        /// <summary>
        /// A callback function which will be called every time a keyboard or mouse activity detected.
        /// <see cref="WinApi.HookCallback"/>
        /// </summary>
        protected int HookCallback(int nCode, Int32 wParam, IntPtr lParam)
        {
            if (nCode == 0)
            {
                bool shouldProcess = ProcessCallback(wParam, lParam);

                if (!shouldProcess)
                {
                    return -1;
                }
            }

            return CallNextHook(nCode, wParam, lParam);
        }

        private int CallNextHook(int nCode, int wParam, IntPtr lParam)
        {
            return HookNativeMethods.CallNextHookEx(HookHandle, nCode, wParam, lParam);
        }

        /// <summary>
        /// Subscribes to the hook and starts firing events.
        /// </summary>
        /// <exception cref="System.ComponentModel.Win32Exception"></exception>
        public void Start()
        {
            if (Enabled)
            {
                throw new InvalidOperationException("Hook listener is already started. Call Stop() method first or use Enabled property.");
            }

            HookCallbackReferenceKeeper = new HookCallback(HookCallback);
            try
            {
                HookHandle = m_Hooker.Subscribe(GetHookId(), HookCallbackReferenceKeeper);
            }
            catch (Exception)
            {
                HookCallbackReferenceKeeper = null;
                HookHandle = 0;
                throw;
            }
        }

        /// <summary>
        /// Unsubscribes from the hook and stops firing events.
        /// </summary>
        /// <exception cref="System.ComponentModel.Win32Exception"></exception>
        public void Stop()
        {
            try
            {
                m_Hooker.Unsubscribe(HookHandle);
            }
            finally
            {
                HookCallbackReferenceKeeper = null;
                HookHandle = 0;
            }
        }

        /// <summary>
        /// Enables you to switch from application hooks to global hooks and vice versa on the fly
        /// without unsubscribing from events. Component remains enabled or disabled state after this call as it was before.
        /// </summary>
        /// <param name="hooker">An AppHooker or GlobalHooker object.</param>
        public void Replace(Hooker hooker)
        {
            bool rememberEnabled = Enabled;
            Enabled = false;
            m_Hooker = hooker;
            Enabled = rememberEnabled;
        }

        /// <summary>
        /// Override to deliver correct id to be used for <see cref="HookNativeMethods.SetWindowsHookEx"/> call.
        /// </summary>
        /// <returns></returns>
        protected abstract int GetHookId();

        /// <summary>
        /// Gets or Sets the enabled status of the Hook.
        /// </summary>
        /// <value>
        /// True - The Hook is presently installed, activated, and will fire events.
        /// <para>
        /// False - The Hook is not part of the hook chain, and will not fire events.
        /// </para>
        /// </value>
        public bool Enabled
        {
            get { return HookHandle != 0; }
            set
            {
                bool mustEnable = value;
                if (mustEnable)
                {
                    if (!Enabled) 
                    {
                        Start();
                    }
                }
                else
                {
                    if (Enabled)
                    {
                        Stop();
                    }
                }
            }
        }

        /// <summary>
        /// Release delegates, unsubscribes from hooks.
        /// </summary>
        /// <filterpriority>2</filterpriority>
        public void Dispose()
        {
            Dispose(true);
            GC.SuppressFinalize(this);
        }

        /// <summary>
        /// Method to be used from Dispose and Finalizer.
        /// Override this method to release subclass specific references.
        /// </summary>
        protected virtual void Dispose(bool isDisposing)
        {
            if (isDisposing)
            {
                Stop();
            }
            else
            {
                if (HookHandle != 0)
                {
                    HookNativeMethods.UnhookWindowsHookEx(HookHandle);
                }
            }
        }

        /// <summary>
        /// Unsubscribes from global hooks skipping error handling.
        /// </summary>
        ~BaseHookListener()
        {
            Dispose(false);
        }
    }
}