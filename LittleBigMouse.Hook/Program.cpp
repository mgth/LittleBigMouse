#include "Framework.h"

#include <tlhelp32.h>
#include <Psapi.h>

#include "Daemon/LittleBigMouseDaemon.h"
#include "Engine/MouseEngine.h"
#include "Hook/Hooker.h"
#include "Logger/Logger.h"
#include "Remote/RemoteServerSocket.h"

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

//int main(int argc, char *argv[]){
int APIENTRY wWinMain(_In_ HINSTANCE hInstance,
                     _In_opt_ HINSTANCE hPrevInstance,
                     _In_ LPWSTR    lpCmdLine,
                     _In_ int       nCmdShow){

    LOG_TRACE("<main> : " << __TIME__ );


#if defined(_use_mutex)
	constexpr LPCWSTR szUniqueNamedMutex = L"LittleBigMouse_Daemon";

	HANDLE hHandle = CreateMutex(nullptr, TRUE, szUniqueNamedMutex);
	if( ERROR_ALREADY_EXISTS == GetLastError() )
	{
	  // Program already running somewhere
		std::cerr << "Program already running\n";
        if(hHandle)
        {
		    CloseHandle (hHandle);
        }
		return(1); // Exit program
	}

    ShowWindow( GetConsoleWindow(), SW_HIDE );
#endif

    SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2 );

    RemoteServerSocket server;
    MouseEngine engine;
    auto hook = Hooker();

    auto p = GetParentProcess();

    // Test if daemon was started from UI
    bool uiMode = p.find("LittleBigMouse") != std::string::npos;

	// TODO: Add command line argument to force UI mode
	//for(int i=0; i<argc; i++)
	//{
	//	if(strcmp(argv[i], "--ignore_current") == 0)
	//	{
	//    	uiMode = true;
	//		break;
	//	}
	//}

    auto daemon = LittleBigMouseDaemon( &server , &engine, &hook );
    ///auto daemon = LittleBigMouseDaemon( nullptr , nullptr, &hook );

    if(uiMode)
    {
        LOG_TRACE("Starting in UI mode");
		daemon.Run("");
	}
	else
	{
		LOG_TRACE("Starting in Daemon mode\n");
		daemon.Run(R"(.\Mgth\LittleBigMouse\Current.xml)");
	}

#if defined(_use_mutex)
    if(hHandle)
    {
        ReleaseMutex (hHandle);
	    CloseHandle (hHandle);
    }
#endif

    LOG_CLOSE;

#if defined(_DEBUG)
    system("pause");
#endif
	return 0;
}
