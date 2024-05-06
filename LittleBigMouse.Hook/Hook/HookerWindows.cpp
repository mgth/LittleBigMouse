#include "Hooker.h"

// TODO: this seems to require a DLL to be injected into the target process

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
	if (!hook) return CallNextHookEx(nullptr, nCode, wParam, lParam);

	if (nCode == WM_DISPLAYCHANGE)
	{
		hook->OnDisplayChanged();

		LOG_TRACE("<HookerWindows:WM_DISPLAYCHANGE>");
	}
	if (nCode == WM_SETTINGCHANGE && wParam == SPI_SETWORKAREA)
	{
		hook->OnDisplayChanged();

		LOG_TRACE("<HookerWindows:WM_SETTINGCHANGE>");
	}

	return CallNextHookEx(hook->_displayHookId, nCode, wParam, lParam);
}


