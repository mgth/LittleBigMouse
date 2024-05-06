#include "Hooker.h"

#include "Logger/Logger.h"
#include "Remote/RemoteServer.h"

std::atomic<Hooker*> Hooker::_instance = nullptr;

void DoSetPriority(const Priority priority)
{
	const auto process = GetCurrentProcess();

	LOG_TRACE("<Hook:SetPriority> : " << priority);

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

void Hooker::DoHook()
{
	if (_instance != nullptr)
	{
			LOG_DEBUG("<Hook:Already hooked>");
			return;
	}

	_instance.store(this);

	if(_hookMouse)
	{
		HookMouse();
	}

	HookFocusEvent();
	HookEventSystemDesktopSwitch();
	HookDisplayChange();

}

void Hooker::DoUnhook()
{
	UnhookMouse();

	UnhookFocusEvent();
	//UnhookEventSystemDesktopSwitch();
	UnhookDisplayChange();

	_instance.store(nullptr);
}

int Hooker::Loop()
{
    MSG msg;

	LOG_TRACE("<Hook:Start>");


	auto ret = GetMessage(&msg, nullptr, 0, 0);
    while (ret)
    {
//        if (!TranslateAccelerator(msg.hwnd, hAccelTable, &msg))
        TranslateMessage(&msg);
        DispatchMessage(&msg);

		if(ret == -1)
		{
			LOG_DEBUG("<Hook:Error>");
			(int)msg.wParam;
		}
		ret = GetMessage(&msg, nullptr, 0, 0);
    }

    return (int)msg.wParam;
}


void Hooker::Loop2()
{
	LOG_TRACE("<Hook:Start>");

    MSG msg;
	int ret = PeekMessage(&msg, nullptr, 0, 0, PM_REMOVE);
	if(ret == 0)
	{
		ret = GetMessage(&msg, nullptr, 0, 0);
	}

	//while we do not close our application
	while (ret >= 0 && msg.message != WM_QUIT)
	{
		LOG_TRACE("msg : " << msg.message);

		TranslateMessage(&msg);
		DispatchMessage(&msg);

		ret = GetMessage(&msg, nullptr, 0, 0);
	}

	if(ret == -1)
	{
		LOG_DEBUG("<Hook:Error>");
	}
}

void Hooker::RunThread()
{
	_currentThreadId = GetCurrentThreadId();

	while(!Stopping())
	{
		DoSetPriority(_priority);
		LOG_TRACE("SetPriority");


		DoHook();
		LOG_TRACE("Hook");

		Loop();
		LOG_TRACE("Loop");

		DoUnhook();
		LOG_TRACE("Unhook");

		DoSetPriority(Below);
		LOG_TRACE("SetPriority");
	}

	LOG_TRACE("<Hook:Stopped>");
}


void Hooker::QuitLoop() const
{
    if (PostThreadMessage(_currentThreadId, WM_QUIT, 0, 0))
    {
        LOG_TRACE("<hook:quit>");
    }
}

void Hooker::DoStop()
{
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
    LOG_TRACE("<hook:DisplayChanged2>");

	const auto hook = Instance();
	if (!hook) return CallNextHookEx(nullptr, nCode, wParam, lParam);

	hook->OnDisplayChanged();

	return CallNextHookEx(hook->_displayHookId, nCode, wParam, lParam);
}

LRESULT __stdcall Hooker::IniChangedCallback(const int nCode, const WPARAM wParam, const LPARAM lParam)
{
    LOG_TRACE("<hook:IniChanged>");

	const auto hook = Instance();
	if (!hook) return CallNextHookEx(nullptr, nCode, wParam, lParam);

	//if (nCode >= 0 && lParam != NULL && (wParam & WM_DISPLAYCHANGE) != 0)
	//{
	//	hook && hook->OnIniChanged.fire();
	//}

	return CallNextHookEx(hook->_displayHookId, nCode, wParam, lParam);
}


