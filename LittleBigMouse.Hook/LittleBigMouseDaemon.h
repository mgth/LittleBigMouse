#pragma once
#include <string>

#include "MouseHookerWindowsHook.h"
#include "RemoteServer.h"
#include "SocketClient.h"
#include "tinyxml2.h"

class LittleBigMouseDaemon
{
	MouseHooker* _hook;
	MouseEngine* _engine;
	RemoteServer* _remoteServer;

public:

	LittleBigMouseDaemon(MouseHooker& hook, RemoteServer& server, MouseEngine& engine);

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

