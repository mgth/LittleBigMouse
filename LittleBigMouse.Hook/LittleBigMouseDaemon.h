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
	void ReceiveLoadMessage(tinyxml2::XMLElement* root) const;
	void ReceiveCommandMessage(tinyxml2::XMLElement* root) const;
	void ReceiveMessage(tinyxml2::XMLElement* root) const;

	void ReceiveMessage(const std::string& m) const;
};

