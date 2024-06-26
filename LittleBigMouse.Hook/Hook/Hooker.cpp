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
		DoSetPriority(_priority);
		HookMouse();
	}
	else
	{
		DoSetPriority(_priorityUnhooked);
	}

	HookFocusEvent();
	HookEventSystemDesktopSwitch();
	HookDisplayChange();

}	


void Hooker::DoUnhook()
{
	UnhookMouse();

	UnhookFocusEvent();
	UnhookEventSystemDesktopSwitch();
	UnhookDisplayChange();

	DoSetPriority(_priorityUnhooked);

	_instance.store(nullptr);
}

bool Hooker::PumpMessages()
{
    MSG msg;

	LOG_TRACE("<Hook:Start>");

	auto ret = GetMessage(&msg, nullptr, 0, 0);
    while (ret>=0)
    {
		if (msg.message == WM_QUIT) {
			LOG_TRACE("<Hook:WM_QUIT>");
			return false;
		}
		if (msg.message == WM_BREAK_LOOP) {
			LOG_TRACE("<Hook:WM_BREAK_LOOP>");
			return true;
		}

//        if (!TranslateAccelerator(msg.hwnd, hAccelTable, &msg))
        TranslateMessage(&msg);
        DispatchMessage(&msg);

		ret = GetMessage(&msg, nullptr, 0, 0);
    }
	if(ret == -1)
	{
		LOG_DEBUG("<Hook:Error>");
		return false;
	}

    return false;
}



void Hooker::Loop()
{
	_currentThreadId = GetCurrentThreadId();

	bool stopping = false;
	while(!stopping)
	{
		DoHook();

		stopping = !PumpMessages();

		DoUnhook();
	}

	LOG_TRACE("<Hook:Stopped>");
}


void Hooker::BreakLoop() const
{
    if (PostThreadMessage(_currentThreadId, WM_BREAK_LOOP, 0, 0))
    {
        LOG_TRACE("<Hook:BreakLoop>");
    }
}

bool Hooker::Hooked() const
{
	return _hookMouse && _mouseHookId;
}

void Hooker::Hook()
{
	_hookMouse = true;
	BreakLoop();
}

void Hooker::Unhook()
{
	_hookMouse = false;
	BreakLoop();
}

void Hooker::Quit() const
{
    if (PostThreadMessage(_currentThreadId, WM_QUIT, 0, 0))
    {
        LOG_TRACE("<Hook:BreakLoop>");
    }
}
