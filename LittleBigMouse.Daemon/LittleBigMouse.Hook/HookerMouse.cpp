#include "Hooker.h"

#include "HookMouseEventArg.h"
#include "Point.h"
#include "MouseEngine.h"
#include "RemoteServer.h"

Hooker* Hooker::_instance = nullptr;

void Hooker::HookMouse()
{
	_currentThreadId = GetCurrentThreadId();
	_mouseHookId = SetWindowsHookEx(WH_MOUSE_LL, &Hooker::MouseCallback, nullptr, 0);
}

void Hooker::UnhookMouse()
{
	if (_mouseHookId && UnhookWindowsHookEx(_mouseHookId))
	{
		_mouseHookId = nullptr;
	}
}

void SetPriority(const Priority priority)
{
	switch(priority)
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
	}
}

void Hooker::Loop()
{
	_instance = this;

	OnMessage.fire("<DaemonMessage><State>Running</State></DaemonMessage>\n");
#if defined(_DEBUG)
	std::cout << "<Hook:Start>\n";
#endif
    MSG msg;
	int ret = PeekMessage(&msg, nullptr, 0, 0, PM_REMOVE);
	if(ret == 0)
	{
		ret = GetMessage(&msg, nullptr, 0, 0);
	}

	//while we do not close our application
	while (ret > 0 && msg.message != WM_QUIT && !Stopping)
	{
		TranslateMessage(&msg);
		DispatchMessage(&msg);

		ret = GetMessage(&msg, nullptr, 0, 0);
	}
	if(ret == -1)
	{
		#if defined(_DEBUG)
		std::cout << "<Hook:Error>";
		#endif
	}

	Stopping = false;

	auto p = MouseEventArg(geo::Point<long>(0,0));
	p.Running = false;
	OnMouseMove.fire(p);

	OnMessage.fire("<DaemonMessage><State>Stopped</State></DaemonMessage>\n");
#if defined(_DEBUG)
	std::cout << "<Hook:Stopped>\n";
#endif
}

void Hooker::RunThread()
{
	SetPriority(_priority);

	HookMouse();

	Loop();

	UnhookMouse();

	SetPriorityClass(GetCurrentProcess(), NORMAL_PRIORITY_CLASS);
}

void Hooker::DoStop()
{
	Stopping = true;
	
    if (PostThreadMessage(_currentThreadId, WM_QUIT, 0, 0))
    {
		#if defined(_DEBUG)
        std::cout << "<hook:quit>" << std::endl;
		#endif
    }
}

void Hooker::OnStopped()
{
}

bool Hooker::Hooked() const
{
	return _mouseHookId;
}

LRESULT __stdcall Hooker::MouseCallback(const int nCode, const WPARAM wParam, const LPARAM lParam)
{
	const auto hook = Instance();

	static auto previousLocation = geo::Point<long>();
	const auto pMouse = reinterpret_cast<MSLLHOOKSTRUCT*>(lParam);

	if (nCode >= 0 && lParam != NULL && (wParam & WM_MOUSEMOVE) != 0)
	{
		const auto location = geo::Point<long>(pMouse->pt.x,pMouse->pt.y);

		if ( previousLocation != location)
		{
			previousLocation = location;

			MouseEventArg p = location;

			hook->OnMouseMove.fire(p);

			if (p.Handled) return 1;
			//if (p.Handled) return -1;
		}
	}

    return CallNextHookEx(hook->_mouseHookId, nCode, wParam, lParam);
}





