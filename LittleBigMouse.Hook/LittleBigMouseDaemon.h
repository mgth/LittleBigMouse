#pragma once
#include <string>

#include "MouseEngine.h"
#include "RemoteServer.h"
#include "SocketClient.h"

class LittleBigMouseDaemon
{
	MouseEngine* _engine;
	RemoteServer* _remoteServer;

public:

	LittleBigMouseDaemon(RemoteServer& server, MouseEngine& engine);

	void Run() const;

	~LittleBigMouseDaemon();

	void ReceiveLoadMessage(tinyxml2::XMLElement* root) const;
	std::string GetStateMessage() const;
	void ReceiveCommandMessage(tinyxml2::XMLElement* root, RemoteClient* client) const;
	void ReceiveMessage(tinyxml2::XMLElement* root, RemoteClient* client) const;

	void ReceiveMessage(const std::string& m, RemoteClient* client) const;

	void LoadFromCurrentFile() const;
	void LoadFromFile(const std::string& path) const;
	void LoadFromFile(const std::wstring& path) const;
};

