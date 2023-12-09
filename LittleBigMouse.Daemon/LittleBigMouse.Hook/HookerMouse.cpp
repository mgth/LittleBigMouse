#include "Hooker.h"
#include <iostream>
#include <MouseEngine.h>

void Hooker::HookMouse()
{
		_mouseHookId = SetWindowsHookEx(WH_MOUSE_LL, &Hooker::MouseCallback, nullptr, 0);

		if(_mouseHookId)
		{
			OnHooked.fire();
			#if defined(_DEBUG)
			std::cout << "<Hook:HookMouse>\n";
			#endif
		}
		else
		{
			OnUnhooked.fire();
			#if defined(_DEBUG)
			std::cout << "<Hook:HookMouse FAILED>\n";
			#endif
		}
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

			#if defined(_DEBUG)
			std::cout << "<Hook:UnhookMouse>\n";
			#endif
		}
		else
		{
			_mouseHookId = nullptr;

			#if defined(_DEBUG)
			std::cout << "<Hook:UnhookMouse FAILED>\n";
			#endif
		}
		OnUnhooked.fire();
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
		}
	}

    return CallNextHookEx(hook->_mouseHookId, nCode, wParam, lParam);
}
