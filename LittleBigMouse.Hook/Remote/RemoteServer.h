#pragma once
#include "framework.h"

#include <atomic>
#include <string>

#include "SignalSlot.h"
#include "RemoteClient.h"
#include "Thread/ThreadHost.h"

class RemoteClient;
class ClientMessage;
class LittleBigMouseDaemon;

class RemoteServer : public ThreadHost
{
protected:
	bool _isRunning = false;

public:
	SIGNAL<void(const std::string&, RemoteClient* client)> OnMessage;

	virtual ~RemoteServer() = default;

	virtual void Send(const std::string& message, RemoteClient* client) = 0;

	virtual void WaitForReady(int delay) const = 0;

	bool IsRunning() const { return _isRunning; }
};

