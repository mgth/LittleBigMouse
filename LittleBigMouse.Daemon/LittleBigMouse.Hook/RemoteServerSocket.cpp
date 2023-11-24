#include <winsock2.h>
#include <ws2tcpip.h>
#include <cstdio>
#pragma comment(lib, "Ws2_32.lib")


#include "RemoteServerSocket.h"
#include "LittleBigMouseDaemon.h"
#include "RemoteClient.h"
#include <iostream>

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

	_socket = socket(AF_INET, SOCK_STREAM, 0);

	std::cout << "Socket.\n";

    if(bind(_socket, reinterpret_cast<SOCKADDR*>(&sin), sizeof(sin))==0)
    {
	    if(listen(_socket, 0)==0)
	    {
		    while(!_stop) 
		    {
		        int sinSize = sizeof(csin);
		        const auto csock = accept(_socket, reinterpret_cast<SOCKADDR*>(&csin), &sinSize);
		        if(csock != INVALID_SOCKET)
		        {
					auto c = new RemoteClient(this, csock);
					_lock.lock();
						_clients.push_back(c);
					_lock.unlock();

					c->Start();

					//immediately inform client of current state
					OnMessage.fire("",c);
		        }
		    }

			_lock.lock();
			const std::vector clients = _clients;
			_lock.unlock();

			while(!_clients.empty())
			{
				const auto c = clients.back();
				c->Stop();
			}
	    }
	    else
	    {
		    std::cout << "Listen failed.\n";
	    }
    }
    closesocket(_socket);
    WSACleanup();
}

void RemoteServerSocket::DoStop()
{
	RemoteServer::DoStop();
	if(_socket!=0)
		closesocket(_socket);
}

void RemoteServerSocket::ReceiveMessage(const std::string& m, RemoteClient* client) 
{
	OnMessage.fire(m,client);
}

void RemoteServerSocket::Remove(RemoteClient* remoteClient)
{
	_lock.lock();

	remoteClient->Stop();
	std::erase(_clients, remoteClient);
	delete remoteClient;

	_lock.unlock();
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
