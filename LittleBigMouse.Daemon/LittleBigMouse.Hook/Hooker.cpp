#include "Hooker.h"

#include "HookMouseEventArg.h"
#include "Point.h"
#include "MouseEngine.h"
#include "RemoteServer.h"

Hooker* Hooker::_instance = nullptr;

void DoSetPriority(const Priority priority)
{
	const auto process = GetCurrentProcess();

	#if defined(_DEBUG)
	std::cout << "<Hook:SetPriority> " << priority <<"\n";
	#endif

	switch(priority)
	{
	case Idle:
		SetPriorityClass(process, IDLE_PRIORITY_CLASS);
		break;
	case Below:
		SetPriorityClass(process, BELOW_NORMAL_PRIORITY_CLASS);
		break;
	case Normal:
		SetPriorityClass(process, NORMAL_PRIORITY_CLASS);
		break;
	case Above:
		SetPriorityClass(process, ABOVE_NORMAL_PRIORITY_CLASS);
		break;
	case High:
		SetPriorityClass(process, HIGH_PRIORITY_CLASS);
		break;
	case Realtime:
		SetPriorityClass(process, REALTIME_PRIORITY_CLASS);
		break;
	}
}

Hooker::Hooker()
{
	_instance = this;
}

void Hooker::DoHook()
{
	if(_hookMouse)
	{
		HookMouse();
	}

	HookFocusEvent();
	HookEventSystemDesktopSwitch();
	HookDisplayChange();
}

void Hooker::Loop()
{
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
	while (ret >= 0 && msg.message != WM_QUIT)
	{
		#if defined(_DEBUG)
		std::cout << "msg" << msg.message << "\n";
		#endif


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
}

void Hooker::RunThread()
{
	_currentThreadId = GetCurrentThreadId();

	while(_run)
	{
		DoSetPriority(_priority);

		DoHook();

		Loop();

		DoUnhook();

		DoSetPriority(Below);
	}

	#if defined(_DEBUG)
		std::cout << "<Hook:Stopped>\n";
	#endif
}

void Hooker::DoUnhook()
{
	UnhookMouse();

	UnhookFocusEvent();
	UnhookEventSystemDesktopSwitch();
	UnhookDisplayChange();
}

void Hooker::QuitLoop()
{
    if (PostThreadMessage(_currentThreadId, WM_QUIT, 0, 0))
    {
		#if defined(_DEBUG)
        std::cout << "<hook:quit>" << std::endl;
		#endif
    }
}

void Hooker::DoStop()
{
	_run = false;
	QuitLoop();
}

void Hooker::OnStopped()
{
}

bool Hooker::Hooked() const
{
	return _hookMouse && _mouseHookId;
}


LRESULT __stdcall Hooker::DisplayChangedCallback(const int nCode, const WPARAM wParam, const LPARAM lParam)
{
	#if defined(_DEBUG)
        std::cout << "<hook:DisplayChanged2>\n";
	#endif

	const auto hook = Instance();

	Instance()->OnDisplayChanged.fire();

	//if (nCode >= 0 && lParam != NULL && (wParam & WM_DISPLAYCHANGE) != 0)
	//{
	//	hook->OnDisplayChanged.fire();
	//}

	return CallNextHookEx(hook->_displayHookId, nCode, wParam, lParam);
}

LRESULT __stdcall Hooker::IniChangedCallback(const int nCode, const WPARAM wParam, const LPARAM lParam)
{
	#if defined(_DEBUG)
        std::cout << "<hook:IniChanged>\n";
	#endif

	const auto hook = Instance();

	//if (nCode >= 0 && lParam != NULL && (wParam & WM_DISPLAYCHANGE) != 0)
	//{
	//	hook->OnIniChanged.fire();
	//}

	return CallNextHookEx(hook->_displayHookId, nCode, wParam, lParam);
}


