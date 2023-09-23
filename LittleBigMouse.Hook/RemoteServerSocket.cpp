#include <winsock2.h>
#include <ws2tcpip.h>
#include <stdio.h>
#pragma comment(lib, "Ws2_32.lib")


#include "RemoteServerSocket.h"
#include "LittleBigMouseDaemon.h"
#include "SocketClient.h"

class SocketClient;

void RemoteServerSocket::RunThread()
{
	WSADATA WSAData;
	SOCKADDR_IN csin{};

    WSAStartup(MAKEWORD(2,0), &WSAData);

    SOCKADDR_IN sin;
	sin.sin_addr.s_addr    = INADDR_ANY; 
	sin.sin_family        = AF_INET;
	sin.sin_port        = htons(25196);

	const SOCKET sock = socket(AF_INET, SOCK_STREAM, 0);

	std::cout << "Socket.\n";

    if(bind(sock, reinterpret_cast<SOCKADDR*>(&sin), sizeof(sin))==0)
    {
	    if(listen(sock, 0)==0)
	    {
		    while(!_stop) 
		    {
		        int sinSize = sizeof(csin);
		        const auto csock = accept(sock, reinterpret_cast<SOCKADDR*>(&csin), &sinSize);
		        if(csock != INVALID_SOCKET)
		        {
					auto c = new SocketClient(this, csock);
		            _clients.push_back(c);
					c->Start();
		        }
		    }
			while(!_clients.empty())
			{
				const auto c = _clients.back();
				_clients.pop_back();
				c->Stop();
				delete c;
			}
	    }
    }
    closesocket(sock);
    WSACleanup();

}

void RemoteServerSocket::ReceiveMessage(const std::string& m) const
{
	_daemon->ReceiveMessage(m);
}

void RemoteServerSocket::Send(const std::string& message) const
{
    for(const auto client : _clients)
    {
        if(client)
			client->Send(message);
    }
}
