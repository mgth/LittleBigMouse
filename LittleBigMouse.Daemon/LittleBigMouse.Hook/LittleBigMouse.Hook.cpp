#include <Windows.h>
#include <cstdio>
#include <tlhelp32.h>
#include <Psapi.h>

#include "LittleBigMouseDaemon.h"
#include "MouseEngine.h"
#include "Hooker.h"
#include "RemoteServerSocket.h"

DWORD GetParentPid(const DWORD pid)
{
    HANDLE h = nullptr;
    PROCESSENTRY32 pe = { 0 };
    DWORD ppid = 0;
    pe.dwSize = sizeof(PROCESSENTRY32);
    h = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
    if( Process32First(h, &pe)) 
    {
        do 
        {
            if (pe.th32ProcessID == pid) 
            {
                ppid = pe.th32ParentProcessID;
                break;
            }
        } while( Process32Next(h, &pe));
    }
    CloseHandle(h);
    return (ppid);
}

DWORD GetProcessName(const DWORD pid, LPWSTR fname, DWORD size)
{
    HANDLE h = nullptr;
    DWORD e = 0;
	h = OpenProcess
    (
    PROCESS_QUERY_INFORMATION | PROCESS_VM_READ,
    FALSE,
    pid
    );

	if (h) 
    {
        if (GetModuleFileNameEx(h, nullptr, fname, size) == 0)
        {
        	e = GetLastError();
        }
        CloseHandle(h);
    }
    else
    {
        e = GetLastError();
    }
    return e;
}

std::string GetParentProcess()
{
	wchar_t fname[MAX_PATH] = {0};
	const DWORD pid = GetCurrentProcessId();
	const DWORD ppid = GetParentPid(pid);
    DWORD e = GetProcessName(ppid, fname, MAX_PATH);
	std::wstring ws(fname);
	return std::string(ws.begin(), ws.end());
}

int main(int argc, char *argv[]){

    //#if !defined(_DEBUG)
	constexpr LPCWSTR szUniqueNamedMutex = L"LittleBigMouse_Daemon";

	HANDLE hHandle = CreateMutex(nullptr, TRUE, szUniqueNamedMutex);
	if( ERROR_ALREADY_EXISTS == GetLastError() )
	{
	  // Program already running somewhere
        #if defined(_DEBUG)
		std::cout << "Program already running\n";
		#endif
        if(hHandle)
        {
		    CloseHandle (hHandle);
        }
		return(1); // Exit program
	}

    #if !defined(_DEBUG)
    ShowWindow( GetConsoleWindow(), SW_HIDE );
    #endif

    SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2 );

    RemoteServerSocket server;
    MouseEngine engine;
    Hooker hook;

    auto p = GetParentProcess();

    // Test if daemon was started from UI
    bool uiMode = p.find("LittleBigMouse") != std::string::npos;
    if(!uiMode)
    {
	    for(int i=0; i<argc; i++)
	    {
			if(strcmp(argv[i], "--ignore_current") == 0)
		    {
	    		uiMode = true;
				break;
			}
		}
	}

    auto daemon = LittleBigMouseDaemon(  &server, &engine, &hook );

    if(uiMode)
    {
        #if defined(_DEBUG)
	    std::cout << "Starting in UI mode\n";
        #endif
		daemon.Run("");
	}
	else
	{
        #if defined(_DEBUG)
		std::cout << "Starting in Daemon mode\n";
        #endif
		daemon.Run("(\Mgth\LittleBigMouse\Current.xml)");
	}

    if(hHandle)
    {
        ReleaseMutex (hHandle);
	    CloseHandle (hHandle);
    }
#if defined(_DEBUG)
    system("pause");
#endif
	return 0;
}
