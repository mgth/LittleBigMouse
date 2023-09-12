#pragma once
#include <atomic>
#include <string>

#include "ThreadHost.h"

class LittleBigMouseDaemon;

class RemoteServer : public ThreadHost
{
protected:
	LittleBigMouseDaemon* _daemon = nullptr;
public:
	virtual ~RemoteServer() = default;
	void SetDaemon(LittleBigMouseDaemon* daemon) {_daemon = daemon;}

	virtual void Send(const std::string& message) const = 0;
};

