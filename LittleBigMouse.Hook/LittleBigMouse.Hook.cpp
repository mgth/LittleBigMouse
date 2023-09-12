#include <iostream>

#include "LittleBigMouseDaemon.h"
#include "MouseEngine.h"
#include "MouseHookerWindowsHook.h"
#include "RemoteServerSocket.h"


int main(){
    SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2 );

    RemoteServerSocket server;
    MouseEngine engine;

    const auto daemon = LittleBigMouseDaemon(server, engine);

    daemon.Run();
    
    return 0;
}
