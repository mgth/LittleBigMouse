#pragma once
#include <string>

#include "HookMouseEventArg.h"

constexpr int INPUTBUFFERSIZE = 1024*16;
constexpr int OUTPUTBUFFERSIZE = 1024*16;

class LittleBigMouseDaemon;

class RemoteServer
{
	HANDLE _outputPipe = INVALID_HANDLE_VALUE;
	HANDLE _inputPipe = INVALID_HANDLE_VALUE;

	public:

	void StartListener(std::string name);
	void StartNotifier(std::string name);
	LittleBigMouseDaemon* Daemon = nullptr;

	void Send(const std::string& message) const;
};

