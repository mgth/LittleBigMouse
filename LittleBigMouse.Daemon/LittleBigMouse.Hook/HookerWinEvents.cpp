#include <iostream>
#include <ostream>

#include "Hooker.h"
#include "str.h"

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

void Hooker::HookFocusEvent()
{
	_hEventFocusHook = SetWinEventHook(
        EVENT_OBJECT_FOCUS, 
        EVENT_OBJECT_FOCUS, 
        nullptr, &Hooker::WindowChangeHook, 
        0, 0, 
        WINEVENT_OUTOFCONTEXT);
}

void Hooker::UnhookFocusEvent()
{
	if(_hEventFocusHook && UnhookWinEvent(_hEventFocusHook))
	{
		_hEventFocusHook = nullptr;
	}
}

void Hooker::HookEventSystemDesktopSwitch()
{
	_hEventDesktopHook = SetWinEventHook(
        EVENT_SYSTEM_DESKTOPSWITCH, 
        EVENT_SYSTEM_DESKTOPSWITCH, 
        nullptr, &Hooker::DesktopChangeHook, 
        0, 0, 
        WINEVENT_OUTOFCONTEXT);
}

void Hooker::UnhookEventSystemDesktopSwitch()
{
	if(_hEventDesktopHook && UnhookWinEvent(_hEventDesktopHook))
	{
		_hEventDesktopHook = nullptr;
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
        if (processId != 0) 
        {
	        const auto exePath = GetExecutablePathFromProcessId(processId);

            #if defined(_DEBUG)
            std::cout << "Window: " << (style & WS_MAXIMIZE) << '\n';
            #endif

            if (!exePath.empty()) 
            {
	            #if defined(_DEBUG)
	                wprintf(L"Executable Path: %s\n", exePath.c_str());
	            #endif

                Instance()->OnFocusChanged.fire(to_string(exePath));

            }
	        #if defined(_DEBUG)
        	else 
            {
                wprintf(L"Unable to retrieve the executable path.\n");
            }
	        #endif
        }
	    #if defined(_DEBUG)
    	else 
        {
            wprintf(L"Unable to retrieve the process ID.\n");
        }
	    #endif
    }
}

void CALLBACK Hooker::DesktopChangeHook(
    HWINEVENTHOOK hWinEventHook, 
    DWORD event, HWND hWnd, LONG idObject, 
    LONG idChild, DWORD dwEventThread, DWORD dwmsEventTime)
{
    #if defined(_DEBUG)
    std::cout << "desktop: \n";
    #endif
    Instance()->OnDesktopChanged.fire();
}



