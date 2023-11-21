#include <Windows.h>
#include <stdio.h>
#include <tlhelp32.h>
#include <Psapi.h>

#include "LittleBigMouseDaemon.h"
#include "MouseEngine.h"
#include "MouseHooker.h"
#include "RemoteServerSocket.h"

DWORD getParentPID(DWORD pid)
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

int getProcessName(const DWORD pid, LPWSTR fname, DWORD size)
{
    HANDLE h = nullptr;
    int e = 0;
	h = OpenProcess
	    (
	    PROCESS_QUERY_INFORMATION | PROCESS_VM_READ,
	    FALSE,
	    pid
	    );
	if (h) 
    {
        if (GetModuleFileNameEx(h, nullptr, fname, size) == 0)
            e = GetLastError();
        CloseHandle(h);
    }
    else
    {
        e = GetLastError();
    }
    return (e);
}

std::string getParentProcess()
{
	wchar_t fname[MAX_PATH] = {0};
	const DWORD pid = GetCurrentProcessId();
	const DWORD ppid = getParentPID(pid);
    int e = getProcessName(ppid, fname, MAX_PATH);
	std::wstring ws(fname);
	return std::string(ws.begin(), ws.end());
}


int main(int argc, char *argv[]){

    ShowWindow( GetConsoleWindow(), SW_HIDE );

    SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2 );

    RemoteServerSocket server;
    MouseEngine engine;
    MouseHooker hook;

    auto p = getParentProcess();
    std::cout << p << std::endl;

    bool ignoreCurrent = p.find("LittleBigMouse") != std::string::npos; //.contains("LittleBigMouse");
    if(!ignoreCurrent)
    {
	    for(int i=0; i<argc; i++)
	    {
			if(strcmp(argv[i], "--ignore_current") == 0)
		    {
	    		ignoreCurrent = true;
				break;
			}
		}
	}

    if(ignoreCurrent)
    {
	    std::cout << "Starting in UI mode" << std::endl;
		LittleBigMouseDaemon( &hook, &server, &engine ).Run("");
	}
	else
	{
		std::cout << "Starting in Daemon mode" << std::endl;
		LittleBigMouseDaemon( &hook, &server, &engine ).Run(R"(\Mgth\LittleBigMouse\Current.xml)");
	}
    
    return 0;
}
