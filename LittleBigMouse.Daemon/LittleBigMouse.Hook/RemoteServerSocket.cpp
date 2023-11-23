#include <winsock2.h>
#include <ws2tcpip.h>
#include <stdio.h>
#pragma comment(lib, "Ws2_32.lib")


#include "RemoteServerSocket.h"
#include "LittleBigMouseDaemon.h"
#include "RemoteClient.h"
#include <iostream>

#include "ClientMessage.h"

class RemoteClient;

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
					auto c = new RemoteClient(this, csock);
		            _clients.push_back(c);
					c->Start();
					//immediately inform client of current state
					OnMessage.fire("",c);
		        }
		    }
			while(!_clients.empty())
			{
				const auto c = _clients.back();

				c->Stop();
				delete c;
			}
	    }
    }
    closesocket(sock);
    WSACleanup();

}

void RemoteServerSocket::ReceiveMessage(const std::string& m, RemoteClient* client) 
{
	OnMessage.fire(m,client);
}

void RemoteServerSocket::Remove(RemoteClient* remoteClient)
{
	std::erase(_clients, remoteClient);
	delete remoteClient;
}

void RemoteServerSocket::Send(const std::string& message, RemoteClient* client) const
{
	if(client)
		client->Send(message);
	else
	{
	    for(const auto c : _clients)
	    {
	        if(c) c->Send(message);
	    }
	}
}
