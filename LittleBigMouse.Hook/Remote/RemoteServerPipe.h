#pragma once
#include "Framework.h"

#include <string>

#include "Engine/HookMouseEventArg.h"
#include "RemoteServer.h"

constexpr int BUFFERSIZE = 1024*16;

class RemoteServerPipe final : public RemoteServer
{
	HANDLE _outputPipe = INVALID_HANDLE_VALUE;
	HANDLE _inputPipe = INVALID_HANDLE_VALUE;

	bool StartListener();
	bool StartNotifier();

protected:
    char _inputBuffer[BUFFERSIZE] = {};
	void RunThread() override
	{
		StartListener() && StartNotifier();
	}

public:
	void Send(const std::string& message, RemoteClient* client) override;
};


