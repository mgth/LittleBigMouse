#include "Hooker.h"
#include <iostream>
#include <MouseEngine.h>

void Hooker::HookMouse()
{
		_mouseHookId = SetWindowsHookEx(WH_MOUSE_LL, &Hooker::MouseCallback, nullptr, 0);

		if(_mouseHookId)
		{
			OnMessage.fire("<DaemonMessage><Event>Running</Event></DaemonMessage>\n");

			#if defined(_DEBUG)
			std::cout << "<Hook:HookMouse>\n";
			#endif
		}
		else
		{
			#if defined(_DEBUG)
			std::cout << "<Hook:HookMouse FAILED>\n";
			#endif
		}
//	_iniHookId = SetWindowsHookEx(WM_WININICHANGE, &IniChangedCallback, nullptr, 0);
//	_displayHookId = SetWindowsHookEx(WM_DISPLAYCHANGE, &DisplayChangedCallback, nullptr, 0);
}

void Hooker::UnhookMouse()
{
	if (_mouseHookId)
	{
		if(UnhookWindowsHookEx(_mouseHookId))
		{
			_mouseHookId = nullptr;

			auto p = MouseEventArg(geo::Point<long>(0,0));
			p.Running = false;
			OnMouseMove.fire(p);

			OnMessage.fire("<DaemonMessage><Event>Stopped</Event></DaemonMessage>\n");

			#if defined(_DEBUG)
			std::cout << "<Hook:UnhookMouse>\n";
			#endif

				//SetPriority(_priority);

			//SetPriorityClass(GetCurrentProcess(), NORMAL_PRIORITY_CLASS);
		}
		else
		{
			_mouseHookId = nullptr;

			OnMessage.fire("<DaemonMessage><Event>Stopped</Event></DaemonMessage>\n");

			#if defined(_DEBUG)
			std::cout << "<Hook:UnhookMouse FAILED>\n";
			#endif
		}
	}
}

LRESULT __stdcall Hooker::MouseCallback(const int nCode, const WPARAM wParam, const LPARAM lParam)
{
	//#if defined(_DEBUG)
	//	std::cout << ".";
	//#endif

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
