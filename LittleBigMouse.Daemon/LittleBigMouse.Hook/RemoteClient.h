#pragma once
#include <atomic>
#include <string>
#include <Windows.h>

#include "ThreadHost.h"

class RemoteServerSocket;

class RemoteClient final : public ThreadHost
{
	RemoteServerSocket* _server;
	SOCKET _client;

//	std::atomic<bool> _stop = false;


protected:
    char _inputBuffer[1024*16];
	void RunThread() override;

	void DoStop() override
	{
		_stop = true;
		closesocket(_client);
	}

public:
	RemoteClient(RemoteServerSocket* server, const SOCKET socket): _server(server), _client(socket), _inputBuffer{}
	{
	}

	void Send(const std::string& message);
};

