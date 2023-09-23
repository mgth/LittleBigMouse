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

	std::atomic<bool> _stop = false;


protected:
    char _inputBuffer[1024*16];
	void RunThread() override;

public:
	RemoteClient(RemoteServerSocket* _server, const SOCKET socket):_server(_server),_client(socket)
	{}

	void Send(const std::string& message) const;
};

