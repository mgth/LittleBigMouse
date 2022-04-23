using System;
using System.Runtime.CompilerServices;

namespace HLab.Sys.MouseHooker
{
    public interface IMouseHooker
    {
        //event EventHandler<HookMouseEventArg> MouseMove;
        void Hook();
        void UnHook();
        bool Hooked();

        void SetMouseMoveAction(Action<IMouseHooker,HookMouseEventArg> action);
    }

    public abstract class MouseHooker : IMouseHooker
    {
        //public event EventHandler<HookMouseEventArg> MouseMove;

        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        protected void OnMouseMove(HookMouseEventArg args)
        {
            //MouseMove?.Invoke(this,args);
            MouseMoveAction?.Invoke(this,args);
        }

        public abstract void Hook();
        public abstract void UnHook();
        public abstract bool Hooked();

        protected Action<IMouseHooker, HookMouseEventArg> MouseMoveAction = null;
        public void SetMouseMoveAction(Action<IMouseHooker, HookMouseEventArg> action)
        {
            MouseMoveAction=action;
        }
    }
}
