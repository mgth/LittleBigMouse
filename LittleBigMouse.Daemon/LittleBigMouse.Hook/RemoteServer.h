#pragma once
#include <atomic>
#include <string>

#include "nano_signal_slot.hpp"
#include "RemoteClient.h"
#include "ThreadHost.h"

class RemoteClient;
class ClientMessage;
class LittleBigMouseDaemon;

class RemoteServer : public ThreadHost
{
public:
	Nano::Signal<void(const std::string&, RemoteClient* client)> OnMessage;

	virtual ~RemoteServer() = default;

	virtual void Send(const std::string& message, RemoteClient* client) = 0;

};

