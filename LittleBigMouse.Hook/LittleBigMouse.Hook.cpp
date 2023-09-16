#include <shlobj.h>
#include <iostream>
#include <fstream>
#include <filesystem>
#include <Shlwapi.h>
#pragma comment(lib,"shlwapi.lib")

#include "LittleBigMouseDaemon.h"
#include "MouseEngine.h"
#include "RemoteServerSocket.h"

int main(){
    SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2 );

    RemoteServerSocket server;
    MouseEngine engine;


    const auto daemon = LittleBigMouseDaemon(server, engine);

    //std::filesystem::path path;
    PWSTR szPath;

    if (SUCCEEDED(SHGetKnownFolderPath(FOLDERID_ProgramData, 0, nullptr, &szPath)))
    {
	    std::ifstream startup;

        PathAppend(szPath, TEXT("\\Mgth\\LittleBigMouse\\Startup.xml"));
	    startup.open(szPath, std::ios::in);

	    std::string buffer;
		std::string line;
		while(startup){
			std::getline(startup, line);
		    daemon.ReceiveMessage(line);
		}

	    startup.close();
    }

    daemon.Run();
    
    return 0;
}
