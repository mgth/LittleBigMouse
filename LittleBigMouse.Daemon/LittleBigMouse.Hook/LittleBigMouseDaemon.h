#pragma once
#include <string>

#include "RemoteServer.h"
//#include "nano_signal_slot.hpp"
#include "RemoteClient.h"
#include "tinyxml2.h"

class MouseEngine;
class MouseHooker;

class LittleBigMouseDaemon
{
	MouseHooker* _hook;
	MouseEngine* _engine;
	RemoteServer* _remoteServer;

	//int _onMouseMoveId;
	//int _onMessageId;
	//int _onServerMessageId;

	void ReceiveLoadMessage(tinyxml2::XMLElement* root) const;
	void ReceiveCommandMessage(tinyxml2::XMLElement* root, RemoteClient* client) const;
	void ReceiveMessage(tinyxml2::XMLElement* root, RemoteClient* client) const;
	void SendState(RemoteClient* client) const;

	void ReceiveClientMessage(const std::string& message, RemoteClient* client) const;
	void Send(const std::string& string) const;

	void LoadFromFile(const std::string& path) const;
	void LoadFromFile(const std::wstring& path) const;

public:
	LittleBigMouseDaemon(MouseHooker* hook, RemoteServer* server, MouseEngine* engine);
	~LittleBigMouseDaemon();

	void Run(const std::string& path) const;

};

