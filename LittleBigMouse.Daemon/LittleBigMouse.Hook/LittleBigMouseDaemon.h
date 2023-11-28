#pragma once
#include <string>

#include "RemoteServer.h"
//#include "nano_signal_slot.hpp"
#include "RemoteClient.h"
#include "tinyxml2.h"

class MouseEngine;
class Hooker;

class LittleBigMouseDaemon
{
	RemoteServer* _remoteServer;
	MouseEngine* _engine;
	Hooker* _hook;

	//int _onMouseMoveId;
	//int _onMessageId;
	//int _onServerMessageId;

	void ReceiveLoadMessage(tinyxml2::XMLElement* root) const;
	void ReceiveCommandMessage(tinyxml2::XMLElement* root, RemoteClient* client) const;
	void ReceiveMessage(tinyxml2::XMLElement* root, RemoteClient* client) const;
	void SendState(RemoteClient* client) const;

	void DisplayChanged() const;
	void DesktopChanged() const;

	void FocusChanged(const std::wstring& path) const;

	void ReceiveClientMessage(const std::string& message, RemoteClient* client) const;
	void Send(const std::string& string) const;

	void LoadFromFile(const std::string& path) const;
	void LoadFromFile(const std::wstring& path) const;

public:
	LittleBigMouseDaemon(RemoteServer* server, MouseEngine* engine, Hooker* hook);
	~LittleBigMouseDaemon();

	void Run(const std::string& path) const;

};

