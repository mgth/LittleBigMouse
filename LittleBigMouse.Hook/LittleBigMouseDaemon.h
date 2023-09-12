#pragma once
#include <string>

#include "MouseEngine.h"
#include "RemoteServer.h"

class LittleBigMouseDaemon
{
	MouseEngine* _engine;
	RemoteServer* _remoteServer;

public:

	LittleBigMouseDaemon(RemoteServer& server, MouseEngine& engine);

	void Run() const;

	~LittleBigMouseDaemon();

	void ReceiveMessage(const std::string& m) const;
};

