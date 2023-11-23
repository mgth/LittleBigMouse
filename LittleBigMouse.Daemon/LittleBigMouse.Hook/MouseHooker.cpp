#include "MouseHooker.h"

#include "HookMouseEventArg.h"
#include "Point.h"
#include "MouseEngine.h"
#include "RemoteServer.h"

MouseHooker* MouseHooker::_instance = nullptr;
HWINEVENTHOOK MouseHooker::_hEventHook = nullptr;

LRESULT CALLBACK MessageHandler(HWND hwnd, UINT msg, WPARAM wParam, LPARAM lParam)
{
    if (msg == WM_CUSTOM_MESSAGE)
    {
        // Handle the custom message here
        std::cout << "Received custom message in the current thread!" << std::endl;
    }

    return DefWindowProc(hwnd, msg, wParam, lParam);
}



int MouseHooker::Hook()
{
	_instance = this;
	_currentThreadId = GetCurrentThreadId();
	_hEventHook = SetWinEventHook(EVENT_OBJECT_FOCUS, EVENT_OBJECT_FOCUS, nullptr, &MouseHooker::WindowChangeHook, 0, 0, WINEVENT_OUTOFCONTEXT);
	WNDCLASS wc = { 0 };
    wc.lpfnWndProc = MessageHandler;
    wc.hInstance = GetModuleHandle(nullptr);
    wc.lpszClassName = L"LbmWindowClass";

    RegisterClass(&wc);
    HWND hwnd = CreateWindow(L"LbmWindowClass", NULL, 0, 0, 0, 0, 0, HWND_MESSAGE, NULL, NULL, NULL);

	_mouseHookId = SetWindowsHookEx(WH_MOUSE_LL, &MouseHooker::MouseCallback, nullptr, 0);

	//HMODULE dll = LoadLibrary(L"LittleBigMouse.Hook.Inject.dll");
	//HOOKPROC addr = (HOOKPROC)GetProcAddress(dll, "WindowCallback");
	//WindowHookId = SetWindowsHookEx(WH_CBT, addr, dll, 0);

	_windowHookId = SetWindowsHookEx(WM_CAPTURECHANGED, &MouseHooker::WindowCallback, nullptr, 0);

	OnMessage.fire("<DaemonMessage><State>Running</State></DaemonMessage>\n");

    MSG msg;
	int ret = PeekMessage(&msg, nullptr, 0, 0, PM_REMOVE);

	ret = -1;
	//while we do not close our application
	while (ret!=0 && msg.message != WM_QUIT && !Stopping)
	//while ((ret = PeekMessage(&msg, nullptr, 0, 0, PM_REMOVE) != 0) && msg.message != WM_QUIT && !Stopping)
	{
		if(ret<0)
		{
			std::cout << "err.";
		}
		else
		{
			TranslateMessage(&msg);
			DispatchMessage(&msg);
		}

		ret = GetMessage(&msg, nullptr, 0, 0);
	}



	std::cout << "<Stopped>" << std::endl;

	Stopping = false;

	auto p = MouseEventArg(geo::Point<long>(0,0));
	p.Running = false;
	OnMouseMove.fire(p);

	OnMessage.fire("<DaemonMessage><State>Stopped</State></DaemonMessage>\n");

	if(_hEventHook)
	{
		UnhookWinEvent(_hEventHook);
		_hEventHook = nullptr;
	}

	if (_mouseHookId && UnhookWindowsHookEx(_mouseHookId))
	{
		_mouseHookId = nullptr;

		if (_windowHookId && UnhookWindowsHookEx(_windowHookId))
		{
			_windowHookId = nullptr;
		}
	}

	DestroyWindow(hwnd);

	return static_cast<int>(msg.wParam); //return the messages
}

void MouseHooker::RunThread()
{
	switch(_priority)
	{
	case Idle:
		SetPriorityClass(GetCurrentProcess(), IDLE_PRIORITY_CLASS);
		break;
	case Below:
		SetPriorityClass(GetCurrentProcess(), BELOW_NORMAL_PRIORITY_CLASS);
		break;
	case Normal:
		SetPriorityClass(GetCurrentProcess(), NORMAL_PRIORITY_CLASS);
		break;
	case Above:
		SetPriorityClass(GetCurrentProcess(), ABOVE_NORMAL_PRIORITY_CLASS);
		break;
	case High:
		SetPriorityClass(GetCurrentProcess(), HIGH_PRIORITY_CLASS);
		break;
	case Realtime:
		SetPriorityClass(GetCurrentProcess(), REALTIME_PRIORITY_CLASS);
		break;
	}



	Hook();

	SetPriorityClass(GetCurrentProcess(), NORMAL_PRIORITY_CLASS);
}

void MouseHooker::DoStop()
{
	Stopping = true;
	
    if (PostThreadMessage(_currentThreadId, WM_QUIT, 0, 0))
    {
        std::cout << "<quit>" << std::endl;
    }
}

void MouseHooker::OnStopped()
{
}



bool MouseHooker::Hooked() const
{
	return _mouseHookId;
}


LRESULT __stdcall MouseHooker::MouseCallback(const int nCode, const WPARAM wParam, const LPARAM lParam)
{
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

LRESULT __stdcall MouseHooker::WindowCallback(const int nCode, const WPARAM wParam, const LPARAM lParam)
{
	const auto hook = MouseHooker::Instance();

	if (nCode == WM_CAPTURECHANGED)
	{
		std::cout << "WM_CAPTURECHANGED : " << wParam << "::" << lParam << std::endl;
	}

    return CallNextHookEx(hook->_windowHookId, nCode, wParam, lParam);
}

DWORD GetProcessIdFromWindow(HWND hWnd) {
    DWORD processId;
    GetWindowThreadProcessId(hWnd, &processId);
    return processId;
}

std::wstring GetExecutablePathFromProcessId(DWORD processId) {
    HANDLE hProcess = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, FALSE, processId);
    if (hProcess != nullptr) {
        wchar_t exePath[MAX_PATH];
        DWORD exePathSize = sizeof(exePath) / sizeof(wchar_t);
        
        if (QueryFullProcessImageName(hProcess, 0, exePath, &exePathSize) != 0) {
            CloseHandle(hProcess);
            return std::wstring(exePath);
        }
        CloseHandle(hProcess);
    }
    return L"";
}


void CALLBACK MouseHooker::WindowChangeHook(HWINEVENTHOOK hWinEventHook, DWORD event, HWND hWnd, LONG idObject, LONG idChild, DWORD dwEventThread, DWORD dwmsEventTime)
{
    if (hWnd != nullptr) {
	    const DWORD processId = GetProcessIdFromWindow(hWnd);
	    const LONG style = GetWindowLong(hWnd, GWL_STYLE);
        if (processId != 0) {
	        const std::wstring exePath = GetExecutablePathFromProcessId(processId);
            std::cout << "Window: " << (style & WS_MAXIMIZE) << std::endl;

            if (!exePath.empty()) {
                // Use the executable path as needed
                wprintf(L"Executable Path: %s\n", exePath.c_str());


            } else {
                wprintf(L"Unable to retrieve the executable path.\n");
            }
        } else {
            wprintf(L"Unable to retrieve the process ID.\n");
        }
    }

}

