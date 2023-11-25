#include <iostream>
#include <ostream>

#include "Hooker.h"

DWORD GetProcessIdFromWindow(HWND hWnd) {
    DWORD processId;
    GetWindowThreadProcessId(hWnd, &processId);
    return processId;
}

std::wstring GetExecutablePathFromProcessId(DWORD processId) {
	const HANDLE hProcess = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, FALSE, processId);
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

void Hooker::HookEvent()
{
	_hEventHook = SetWinEventHook(
        EVENT_OBJECT_FOCUS, 
        EVENT_OBJECT_FOCUS, 
        nullptr, &Hooker::WindowChangeHook, 
        0, 0, 
        WINEVENT_OUTOFCONTEXT);
}

void Hooker::UnhookEvent()
{
	if(_hEventHook)
	{
		UnhookWinEvent(_hEventHook);
		_hEventHook = nullptr;
	}
}

void CALLBACK Hooker::WindowChangeHook(
    HWINEVENTHOOK hWinEventHook, 
    DWORD event, HWND hWnd, LONG idObject, 
    LONG idChild, DWORD dwEventThread, DWORD dwmsEventTime)
{
    if (hWnd != nullptr) {
	    const DWORD processId = GetProcessIdFromWindow(hWnd);
	    const LONG style = GetWindowLong(hWnd, GWL_STYLE);
        if (processId != 0) {
	        auto exePath = GetExecutablePathFromProcessId(processId);

            #if defined(_DEBUG)
            std::cout << "Window: " << (style & WS_MAXIMIZE) << '\n';
            #endif

            if (!exePath.empty()) {
                // Use the executable path as needed
                wprintf(L"Executable Path: %s\n", exePath.c_str());

                Instance()->OnWindowsChanged.fire(exePath);

            } else {
                wprintf(L"Unable to retrieve the executable path.\n");
            }
        } else {
            wprintf(L"Unable to retrieve the process ID.\n");
        }
    }
}


