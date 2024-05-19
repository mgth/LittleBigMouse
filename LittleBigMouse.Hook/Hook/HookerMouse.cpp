#include "Hooker.h"

#include "Engine/MouseEngine.h"

void Hooker::HookMouse()
{
	_mouseHookId = SetWindowsHookEx(WH_MOUSE_LL, &Hooker::MouseCallback, nullptr, 0);

	if(_mouseHookId)
	{
		LOG_TRACE("<Hook:HookMouse>");
		OnHooked();
	}
	else
	{
		LOG_DEBUG("<Hook:HookMouse FAILED>");
		OnUnhooked();
	}
}

void Hooker::UnhookMouse()
{
	if (!_mouseHookId) return;

	if(UnhookWindowsHookEx(_mouseHookId))
	{
		LOG_TRACE("<Hook:UnhookMouse>");

		_mouseHookId = nullptr;

		auto p = MouseEventArg(geo::Point<long>(0,0));
		p.Running = false;
		OnMouseMove(p);
	}
	else
	{
		LOG_DEBUG("<Hook:UnhookMouse FAILED>");

		_mouseHookId = nullptr;
	}
	OnUnhooked();
}

LRESULT __stdcall Hooker::MouseCallback(const int nCode, const WPARAM wParam, const LPARAM lParam)
{
	const auto hook = Instance();
	if (!hook) return CallNextHookEx(nullptr, nCode, wParam, lParam);

	static auto previousLocation = geo::Point<long>();
	const auto pMouse = reinterpret_cast<MSLLHOOKSTRUCT*>(lParam);

	if (nCode >= 0 && lParam != NULL && (wParam & WM_MOUSEMOVE) != 0)
	{
		if (const auto location = geo::Point(pMouse->pt.x,pMouse->pt.y); previousLocation != location)
		{
			previousLocation = location;
			auto p = MouseEventArg(location);

			hook->OnMouseMove(p);

			if (p.Handled) return 1;
		}
	}

    return CallNextHookEx(hook->_mouseHookId, nCode, wParam, lParam);
}
