#pragma once
#include "Framework.h"

#include <mutex>
#include <atomic>
#include <condition_variable>
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

	// deferred clip checks
	std::thread _clipThread;
	std::atomic<bool> _stopping{false};
	std::atomic<uint64_t> _focusGen = 0;
	std::mutex _clipMutex;
	std::condition_variable _cv;
	bool _clipPending = false;
	void StartClipWatcher();
	void StopClipWatcher();
	void HandleClipCheck();

	void Connect();
	void Disconnect();
	void ReceiveListenMessage(RemoteClient* client) const;

	void ReceiveLoadMessage(tinyxml2::XMLElement* root) const;
	void ReceiveCommandMessage(tinyxml2::XMLElement* root, RemoteClient* client);
	void ReceiveMessage(tinyxml2::XMLElement* root, RemoteClient* client);
	void SendState(RemoteClient* client) const;

	void DisplayChanged() const;
	void SettingChanged() const;
	void DesktopChanged() const;
	[[nodiscard]] bool Excluded(const std::string& path) const;

	void FocusChanged(const std::string& path);

	void ReceiveClientMessage(const std::string& message, RemoteClient* client);
	void Unhooked() const;
	void Hooked() const;

	void LoadFromFile(const std::string& path);
	void LoadExcluded(const std::string& path);
	void LoadExcluded();
public:
	LittleBigMouseDaemon(RemoteServer* server, MouseEngine* engine, Hooker* hook);

	void Run(const std::string& path);

};

