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

	// list of excluded processes
	std::vector<std::string> _excluded;

	// paused when current process is excluded
	bool _paused = false;

	void Connect();

	void ReceiveLoadMessage(tinyxml2::XMLElement* root) const;
	void ReceiveCommandMessage(tinyxml2::XMLElement* root, RemoteClient* client);
	void ReceiveMessage(tinyxml2::XMLElement* root, RemoteClient* client);
	void SendState(RemoteClient* client) const;

	void DisplayChanged() const;
	void DesktopChanged() const;
	[[nodiscard]] bool Excluded(const std::string& path) const;

	void FocusChanged(const std::string& path);

	void ReceiveClientMessage(const std::string& message, RemoteClient* client);
	void Send() const;

	void LoadFromFile(const std::string& path);
	void LoadExcluded(const std::string& path);

public:
	LittleBigMouseDaemon(RemoteServer* server, MouseEngine* engine, Hooker* hook);
	~LittleBigMouseDaemon();

	void Run(const std::string& path);

};

