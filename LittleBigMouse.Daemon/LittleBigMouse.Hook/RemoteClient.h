#pragma once
#include <atomic>
#include <string>
#include <Windows.h>

#include "ThreadHost.h"

class RemoteServerSocket;

class RemoteClient final : public ThreadHost
{
	RemoteServerSocket* _server;
	SOCKET _socket;

//	std::atomic<bool> _stop = false;


protected:
    char _inputBuffer[1024*16];
	void RunThread() override;

	void DoStop() override
	{
		ThreadHost::DoStop();
		if(_socket)
			shutdown(_socket,2);
		if(_socket)
			closesocket(_socket);
	}

public:
	RemoteClient(RemoteServerSocket* server, const SOCKET socket): _server(server), _socket(socket), _inputBuffer{}
	{
	}

	void Send(const std::string& message);
};

