#pragma once
#include <string>
#include <vector>

#include "RemoteServer.h"

class RemoteClient;

class RemoteServerSocket final : public RemoteServer
{
    std::mutex _lock; 
    std::vector<RemoteClient*> _clients;
    std::vector<RemoteClient*> _deadClients;

    SOCKET _socket = 0;

protected:
	void RunThread() override;
    void DeleteDeadClients();
    void DoStop() override;

public:

	void Send(const std::string& message,RemoteClient* client) override;

    void ReceiveMessage(const std::string& m, RemoteClient* client);
    void Remove(RemoteClient* remoteClient);
};

