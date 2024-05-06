#include "Hooker.h"

#include "Strings/str.h"

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
	static HWND lastHwnd = nullptr;

	auto hook = Instance();
	if (!hook) return;

	if (hWnd != lastHwnd) {
		lastHwnd = hWnd;
	    const DWORD processId = GetProcessIdFromWindow(hWnd);
	    const LONG style = GetWindowLong(hWnd, GWL_STYLE);
        if (processId != 0) 
        {
	        const auto exePath = GetExecutablePathFromProcessId(processId);

            LOG_TRACE("Window: " << hWnd << ((style & WS_MAXIMIZE==WS_MAXIMIZE)?"max_":"std_") << ((style & WS_VISIBLE==WS_VISIBLE)?"visible":"hidden"));

            if (!exePath.empty()) 
            {
                LOG_TRACE("Executable Path: " << ToString(exePath));

                hook->OnFocusChanged(ToString(exePath));

            }
        	else 
            {
                LOG_DEBUG("Unable to retrieve the executable path.");
            }
        }
    	else 
        {
            LOG_DEBUG("Unable to retrieve the process ID.");
        }
    }
}

void CALLBACK Hooker::DesktopChangeHook(
    HWINEVENTHOOK hWinEventHook, 
    DWORD event, HWND hWnd, LONG idObject, 
    LONG idChild, DWORD dwEventThread, DWORD dwmsEventTime)
{
    LOG_TRACE("desktop: ");

	auto hook = Instance();
	if (!hook) return;

    hook->OnDesktopChanged();
}



