#pragma once
#include <atomic>
#include <string>

#include "ThreadHost.h"

class LittleBigMouseDaemon;

class RemoteServer : public ThreadHost
{
public:
	virtual ~RemoteServer() = default;

	virtual void Send(const std::string& message) const = 0;
};

