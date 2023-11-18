#pragma once
#include <atomic>
#include <string>

#include "nano_signal_slot.hpp"
#include "SocketClient.h"
#include "ThreadHost.h"

class RemoteClient;
class ClientMessage;
class LittleBigMouseDaemon;

class RemoteServer : public ThreadHost
{
public:
	Nano::Signal<void(const std::string&, RemoteClient* client)> OnMessage;

	virtual ~RemoteServer() = default;

	virtual void Send(const std::string& message, const RemoteClient* client) const = 0;
};

