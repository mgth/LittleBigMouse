using System;
using System.Runtime.CompilerServices;

namespace HLab.Sys.MouseHooker
{
    public interface IMouseHooker
    {
        //event EventHandler<HookMouseEventArg> MouseMove;
        bool Hook();
        bool UnHook();
        bool Hooked { get; }

        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        public void SetMouseMoveAction(Action<IMouseHooker, HookMouseEventArg> action);
    }

    public abstract class MouseHooker : IMouseHooker
    {
        //public event EventHandler<HookMouseEventArg> MouseMove;

        private static MouseHooker _this;

        public MouseHooker()
        {
            _this = this;

        }

        [MethodImpl(MethodImplOptions.AggressiveInlining)]
        protected static void OnMouseMove(HookMouseEventArg args)
        {
            //MouseMove?.Invoke(this,args);
            _this.MouseMoveAction?.Invoke(_this, args);
        }

        public abstract bool Hook();
        public abstract bool UnHook();
        public abstract bool Hooked { get; }

        protected Action<IMouseHooker, HookMouseEventArg> MouseMoveAction = null;
        public void SetMouseMoveAction(Action<IMouseHooker, HookMouseEventArg> action)
        {
            MouseMoveAction = action;
        }
    }
}
