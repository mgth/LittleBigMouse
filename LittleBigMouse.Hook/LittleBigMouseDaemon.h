#pragma once
#include <string>

#include "MouseEngine.h"
#include "RemoteServer.h"

class LittleBigMouseDaemon
{
	MouseEngine _engine;
	RemoteServer _remoteServer = RemoteServer();


public:

	LittleBigMouseDaemon()
	{
		_remoteServer.Daemon = this;
		_engine.Remote = &_remoteServer;
		_remoteServer.StartNotifier("lbm-daemon-beta");
		_remoteServer.StartListener("lbm-daemon-beta");
	}


	void ReceiveMessage(std::string m);
};

