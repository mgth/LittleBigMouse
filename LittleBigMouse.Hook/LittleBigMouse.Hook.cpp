#include "LittleBigMouseDaemon.h"
#include "MouseEngine.h"
#include "RemoteServerSocket.h"

int main(){
    SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2 );

    RemoteServerSocket server;
    MouseEngine engine;

    LittleBigMouseDaemon(server, engine).Run();
    
    return 0;
}
