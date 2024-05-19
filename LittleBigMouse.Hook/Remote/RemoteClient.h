#pragma once
#include "Framework.h"

#include <string>

#include "Thread/ThreadHost.h"

class RemoteServerSocket;

class RemoteClient final : public ThreadHost
{
	RemoteServerSocket* _server;
	SOCKET _socket;
	bool _listening;

protected:
    char _inputBuffer[1024*16];
	void RunThread() override;
	void DoStop() override;

public:
	RemoteClient(RemoteServerSocket* server, const SOCKET socket): _server(server), _socket(socket), _inputBuffer{}
	{
	}

	void Send(const std::string& message);
	bool Listening() const { return _listening; }
	void Listen() { _listening = true; }
};

