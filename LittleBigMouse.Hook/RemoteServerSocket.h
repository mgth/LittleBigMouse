#pragma once
#include <string>
#include <vector>

#include "RemoteServer.h"

class SocketClient;

class RemoteServerSocket final : public RemoteServer
{
    std::vector<SocketClient*> _clients;

protected:
	void RunThread() override;

public:

	void Send(const std::string& message) const override;

    void ReceiveMessage(const std::string& m) const;

};

