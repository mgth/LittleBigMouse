#include "MouseHooker.h"

#include "HookMouseEventArg.h"
#include "Point.h"
#include "MouseEngine.h"
#include "RemoteServer.h"


int MouseHooker::Hook()
{
	_instance = this;
	MouseHookId = SetWindowsHookEx(WH_MOUSE_LL, &MouseCallback, nullptr, 0);

	HMODULE dll = LoadLibrary(L"LittleBigMouse.Hook.Inject.dll");
	HOOKPROC addr = (HOOKPROC)GetProcAddress(dll, "WindowCallback");
	WindowHookId = SetWindowsHookEx(WH_CBT, addr, dll, 0);

	//WindowHookId = SetWindowsHookEx(WH_CBT, &WindowCallback, nullptr, 0);

	OnMessage.emit("<DaemonMessage><State>Running</State></DaemonMessage>\n");

    MSG msg;

	//while we do not close our application
	while (GetMessage(&msg, nullptr, 0, 0) && msg.message != WM_QUIT && !Stopping)
	{ 
		TranslateMessage(&msg);
		DispatchMessage(&msg);
	}

	Stopping = false;
	OnMessage.emit("<DaemonMessage><State>Stopped</State></DaemonMessage>\n");

	if (UnhookWindowsHookEx(MouseHookId))
	{
		MouseHookId = nullptr;

		if (UnhookWindowsHookEx(WindowHookId))
		{
			WindowHookId = nullptr;
		}
	}

	return static_cast<int>(msg.wParam); //return the messages
}

void MouseHooker::RunThread()
{
	switch(_engine->Layout.Priority)
	{
	case Idle:
		SetPriorityClass(GetCurrentProcess(), IDLE_PRIORITY_CLASS);
		break;
	case Below:
		SetPriorityClass(GetCurrentProcess(), BELOW_NORMAL_PRIORITY_CLASS);
		break;
	case Normal:
		SetPriorityClass(GetCurrentProcess(), NORMAL_PRIORITY_CLASS);
		break;
	case Above:
		SetPriorityClass(GetCurrentProcess(), ABOVE_NORMAL_PRIORITY_CLASS);
		break;
	case High:
		SetPriorityClass(GetCurrentProcess(), HIGH_PRIORITY_CLASS);
		break;
	case Realtime:
		SetPriorityClass(GetCurrentProcess(), REALTIME_PRIORITY_CLASS);
		break;
	default: ;
		SetPriorityClass(GetCurrentProcess(), NORMAL_PRIORITY_CLASS);
	}

	Start();

	SetPriorityClass(GetCurrentProcess(), NORMAL_PRIORITY_CLASS);
}

void MouseHooker::DoStop()
{
	Stopping = true;
}

void MouseHooker::OnStopped()
{
	_engine->Reset();
}



bool MouseHooker::Hooked() const
{
	return MouseHookId;
}


LRESULT __stdcall MouseHooker::MouseCallback(const int nCode, const WPARAM wParam, const LPARAM lParam)
{
	const auto hook = MouseHooker::Instance();

	static auto previousLocation = geo::Point<long>();
	const auto pMouse = reinterpret_cast<MSLLHOOKSTRUCT*>(lParam);

	if (nCode >= 0 && lParam != NULL && (wParam & WM_MOUSEMOVE) != 0)
	{
		const auto location = geo::Point<long>(pMouse->pt.x,pMouse->pt.y);

		if ( previousLocation != location)
		{
			previousLocation = location;

			auto p = MouseEventArg();
			p.Point = location;

			hook->_signal.emit(p);

			//hook->Engine()->OnMouseMove(p);

			if (p.Handled) return 1;
			//if (p.Handled) return -1;
		}
	}

    return CallNextHookEx(hook->MouseHookId, nCode, wParam, lParam);
}

static LRESULT __stdcall WindowCallback(const int nCode, const WPARAM wParam, const LPARAM lParam)
{
	const auto hook = MouseHooker::Instance();

	if (nCode == HCBT_SETFOCUS)
	{
		std::cout << "HCBT_SETFOCUS" << wParam << std::endl;
	}

    return CallNextHookEx(hook->WindowHookId, nCode, wParam, lParam);
}
