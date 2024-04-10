#include <iostream>
#include <ostream>

#include "Hooker.h"

void Hooker::HookDisplayChange()
{
	WNDCLASS wndClass;
    wndClass.lpfnWndProc = DisplayChangeHandler;
    wndClass.hInstance = GetModuleHandle(nullptr);
    wndClass.lpszClassName = L"LittleBigMouse";

    RegisterClass(&wndClass);
    //(lpClassName, lpWindowName, dwStyle, x, y, nWidth, nHeight, hWndParent, hMenu, hInstance, lpParam)
	_hwnd = CreateWindow(
        wndClass.lpszClassName, 
        wndClass.lpszClassName, 
        0, 0, 0, 0, 0, 
        HWND_DESKTOP, 
        NULL, 
        NULL, 
        NULL
    );
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

