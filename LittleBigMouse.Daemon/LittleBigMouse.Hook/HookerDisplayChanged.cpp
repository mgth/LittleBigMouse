#include <iostream>
#include <ostream>

#include "Hooker.h"

void Hooker::HookDisplayChange()
{
	WNDCLASS wc = { 0 };
    wc.lpfnWndProc = DisplayChangeHandler;
    wc.hInstance = GetModuleHandle(nullptr);
    wc.lpszClassName = L"LbmWindowClass";

    RegisterClass(&wc);
	_hwnd = CreateWindow(wc.lpszClassName, NULL, 0, 0, 0, 0, 0, HWND_DESKTOP, NULL, NULL, NULL);
}

void Hooker::UnhookDisplayChange()
{
	DestroyWindow(_hwnd);
}

LRESULT CALLBACK Hooker::DisplayChangeHandler(HWND hwnd, UINT msg, WPARAM wParam, LPARAM lParam)
{
    if (msg == WM_DISPLAYCHANGE)
    {
		Instance()->OnDisplayChanged.fire();
        // Handle the custom message here
        #if defined(_DEBUG)
        std::cout << "<hook:DisplayChanged1>\n";
        #endif
    }

    return DefWindowProc(hwnd, msg, wParam, lParam);
}

