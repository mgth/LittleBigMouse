#include "MouseHookerWindowsHook.h"

#include "HookMouseEventArg.h"
#include "Point.h"

MouseEngine* MouseHookerWindowsHook::_engine;
HHOOK MouseHookerWindowsHook::HookId;
MSG MouseHookerWindowsHook::Msg;
std::atomic_bool MouseHookerWindowsHook::Stop = false;

int MouseHookerWindowsHook::Start(MouseEngine& engine)
{
	_engine = &engine;
	HookId = SetWindowsHookEx(WH_MOUSE_LL, MouseCallback, nullptr, 0);

	_engine->Remote->Send("<DaemonMessage><State>Running</State></DaemonMessage>");

	while (Msg.message != WM_QUIT && !Stop){ //while we do not close our application
		if (PeekMessage(&Msg, nullptr, 0, 0, PM_REMOVE)){
			TranslateMessage(&Msg);
			DispatchMessage(&Msg);
		}
		Sleep(1);
	}

	Stop = false;
	_engine->Remote->Send("<DaemonMessage><State>Stopped</State></DaemonMessage>");

	if (UnhookWindowsHookEx(HookId))
	{
		_engine = nullptr;
		HookId = nullptr;
	}
	return static_cast<int>(Msg.wParam); //return the messages
}

bool MouseHookerWindowsHook::Hooked()
{
	return HookId;
}


LRESULT __stdcall MouseCallback(const int nCode, const WPARAM wParam, const LPARAM lParam)
{
	static POINT old = POINT{0,0};
	const auto pMouse = reinterpret_cast<MSLLHOOKSTRUCT*>(lParam);

	if (nCode >= 0 && lParam != NULL && (wParam & WM_MOUSEMOVE) != 0)
	{
		const POINT pt = pMouse->pt;

		if ( old.x != pt.x || old.y != pt.y)
		{
			old = pt;

			auto p = HookMouseEventArg
			{
				pt,
				false
			};

			MouseHookerWindowsHook::GetEngine()->OnMouseMove(p);

			if (p.Handled) return -1;
		}
	}

    return CallNextHookEx(MouseHookerWindowsHook::HookId, nCode, wParam, lParam);
}
