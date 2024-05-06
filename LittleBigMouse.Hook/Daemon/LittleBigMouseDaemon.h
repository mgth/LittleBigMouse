#pragma once
#include "Framework.h"

#include <string>
#include <vector>

namespace tinyxml2
{
	class XMLElement;
}

class RemoteServer;
class RemoteClient;
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
	void Disconnect();

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
	void LoadExcluded();
public:
	LittleBigMouseDaemon(RemoteServer* server, MouseEngine* engine, Hooker* hook);

	void Run(const std::string& path);

};

