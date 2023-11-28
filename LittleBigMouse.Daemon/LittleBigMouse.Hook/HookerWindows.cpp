#include <iostream>
#include <ostream>

#include "Hooker.h"

// TODO: this seams to require a DLL to be injected into the target process

void Hooker::HookWindows()
{
	//HMODULE dll = LoadLibrary(L"LittleBigMouse.Hook.Inject.dll");
	//HOOKPROC addr = (HOOKPROC)GetProcAddress(dll, "WindowCallback");
	//WindowHookId = SetWindowsHookEx(WH_CBT, addr, dll, 0);

	//_windowHookId = SetWindowsHookEx(WM_CAPTURECHANGED, &Hooker::WindowCallback, nullptr, 0);
	_displayHookId = SetWindowsHookEx(WM_DISPLAYCHANGE, &Hooker::WindowCallback, nullptr, 0);

}

void Hooker::UnhookWindows()
{
	if (_displayHookId && UnhookWindowsHookEx(_displayHookId))
	{
		_displayHookId = nullptr;
	}
}

LRESULT __stdcall Hooker::WindowCallback(const int nCode, const WPARAM wParam, const LPARAM lParam)
{
	const auto hook = Instance();

	if (nCode == WM_DISPLAYCHANGE)
	{
		Hooker::Instance()->OnDisplayChanged.fire();

		#if defined(_DEBUG)
		std::cout << "WM_DISPLAYCHANGE : " << wParam << "::" << lParam << std::endl;
		#endif
	}

	return CallNextHookEx(hook->_displayHookId, nCode, wParam, lParam);
}


