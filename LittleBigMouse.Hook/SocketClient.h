#pragma once
#include <atomic>
#include <string>
#include <winsock2.h>

#include "ThreadHost.h"
class RemoteServerSocket;

constexpr int INPUTBUFFERSIZE = 1024*16;
constexpr int OUTPUTBUFFERSIZE = 1024*16;

class SocketClient final : public ThreadHost
{
	RemoteServerSocket* _server;
	SOCKET _client;

	std::atomic<bool> _stop = false;


protected:
    char _inputBuffer[INPUTBUFFERSIZE];
	void RunThread() override;

public:
	SocketClient(RemoteServerSocket* _server,const SOCKET socket):_server(_server),_client(socket)
	{}

	void Send(const std::string& message) const;
};

