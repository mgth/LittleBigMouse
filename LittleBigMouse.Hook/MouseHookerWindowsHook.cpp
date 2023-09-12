#include "MouseHookerWindowsHook.h"

#include "HookMouseEventArg.h"
#include "Point.h"
#include "MouseEngine.h"
#include "RemoteServer.h"

MouseEngine* MouseHookerWindowsHook::_engine;
HHOOK MouseHookerWindowsHook::HookId;
MSG MouseHookerWindowsHook::Msg;
std::atomic_bool MouseHookerWindowsHook::Stopping = false;

int MouseHookerWindowsHook::Start(MouseEngine& engine)
{
	_engine = &engine;
	HookId = SetWindowsHookEx(WH_MOUSE_LL, MouseCallback, nullptr, 0);

	_engine->Send("<DaemonMessage><State>Running</State></DaemonMessage>\n");

	while (Msg.message != WM_QUIT && !Stopping){ //while we do not close our application
		if (PeekMessage(&Msg, nullptr, 0, 0, PM_REMOVE)){
			TranslateMessage(&Msg);
			DispatchMessage(&Msg);
		}
		Sleep(1);
	}

	Stopping = false;
	_engine->Send("<DaemonMessage><State>Stopped</State></DaemonMessage>\n");

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

void MouseHookerWindowsHook::SetMouseLocation(const geo::Point<long>& location)
{
	SetCursorPos(location.X(),location.Y());
}

geo::Point<long> MouseHookerWindowsHook::GetMouseLocation()
{
	POINT p;
	if(GetCursorPos(&p)) return {p.x,p.y};
	return geo::Point<long>::Empty();
}

void MouseHookerWindowsHook::SetClip(const geo::Rect<long>& r)
{
	const auto rect = RECT{r.Left(),r.Top(),r.Right(),r.Bottom()};
	ClipCursor(&rect);
}

geo::Rect<long> MouseHookerWindowsHook::GetClip()
{
	RECT r;
	if(GetClipCursor(&r))
	{
		return geo::Rect<long>(r.left,r.top,r.right-r.left,r.bottom-r.top);
	}
	return geo::Rect<long>::Empty();
}


LRESULT __stdcall MouseCallback(const int nCode, const WPARAM wParam, const LPARAM lParam)
{
	static auto old = geo::Point<long>();
	const auto pMouse = reinterpret_cast<MSLLHOOKSTRUCT*>(lParam);

	if (nCode >= 0 && lParam != NULL && (wParam & WM_MOUSEMOVE) != 0)
	{
		const auto pt = geo::Point<long>(pMouse->pt.x,pMouse->pt.y);

		if ( old.X() != pt.X() || old.Y() != pt.Y())
		{
			old = pt;

			auto p = HookMouseEventArg();
			p.Point = pt;

			MouseHookerWindowsHook::GetEngine()->OnMouseMove(p);

			if (p.Handled) return -1;
		}
	}

    return CallNextHookEx(MouseHookerWindowsHook::HookId, nCode, wParam, lParam);
}
