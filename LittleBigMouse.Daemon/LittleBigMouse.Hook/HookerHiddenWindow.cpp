#include <iostream>
#include <ostream>

#include "Hooker.h"


LRESULT CALLBACK MessageHandler(HWND hwnd, UINT msg, WPARAM wParam, LPARAM lParam)
{
    if (msg == WM_CUSTOM_MESSAGE)
    {
		//Hooker::Instance()->OnWindowsChanged.fire();
        // Handle the custom message here
        std::cout << "Received custom message in the current thread!\n";
    }

    return DefWindowProc(hwnd, msg, wParam, lParam);
}


void Hooker::HookHiddenWindow()
{
	_instance = this;

	WNDCLASS wc = { 0 };
    wc.lpfnWndProc = MessageHandler;
    wc.hInstance = GetModuleHandle(nullptr);
    wc.lpszClassName = L"LbmWindowClass";

    RegisterClass(&wc);
	const HWND hwnd = CreateWindow(L"LbmWindowClass", NULL, 0, 0, 0, 0, 0, HWND_MESSAGE, NULL, NULL, NULL);
}

void Hooker::UnhookHiddenWindow()
{
	DestroyWindow(_hwnd);
}