#include "RemoteServerSocket.h"
#include "RemoteClient.h"

#pragma comment(lib, "Ws2_32.lib")

void RemoteServerSocket::RunThread()
{
	WSADATA WSAData;
	SOCKADDR_IN csin{};

	auto result = WSAStartup(MAKEWORD(2, 0), &WSAData);
	if (result != 0)
	{
		_isRunning = false;
		LOG_DEBUG("WSAStartup failed.");
		return;
	}

	SOCKADDR_IN sin;
	sin.sin_addr.s_addr = INADDR_ANY;
	sin.sin_family = AF_INET;
	sin.sin_port = htons(25196);

	_socket = socket(AF_INET, SOCK_STREAM, 0);

	LOG_TRACE("<Server:Start>");

	if (bind(_socket, reinterpret_cast<SOCKADDR*>(&sin), sizeof(sin)) == 0)
	{
		if (listen(_socket, 0) == 0)
		{
			_isRunning = true;
			while (!Stopping())
			{
				int sinSize = sizeof(csin);
				const auto csock = accept(_socket, reinterpret_cast<SOCKADDR*>(&csin), &sinSize);
				if (csock != INVALID_SOCKET)
				{
					auto c = new RemoteClient(this, csock);
					_lock.lock();
					_clients.push_back(c);
					_lock.unlock();

					c->Start();

					//immediately inform client of current state
					OnMessage("", &*c);
				}
				else
				{
					LOG_TRACE("<Server:Dead>.");
				}

				DeleteDeadClients();
			}

			while (!_clients.empty())
			{
				_lock.lock();
				const auto c = _clients.back();
				_clients.pop_back();
				_lock.unlock();

				c->Stop();
			}
			DeleteDeadClients();
			LOG_TRACE("<Server:Stopped>");
		}
		else
		{
			LOG_TRACE("Listen failed.");
		}
	}

	if (_socket != 0)
		closesocket(_socket);

	WSACleanup();

	_isRunning = false;
}

void RemoteServerSocket::DeleteDeadClients()
{
	while (!_deadClients.empty())
	{
		_lock.lock();
		const auto c = _deadClients.back();
		_deadClients.pop_back();
		_lock.unlock();

		c->Join();
		delete c;
	}
}

void RemoteServerSocket::DoStop()
{
	RemoteServer::DoStop();
	if (_socket != 0)
		shutdown(_socket, 2);
	if (_socket != 0)
		closesocket(_socket);

	_socket = 0;
}

void RemoteServerSocket::ReceiveMessage(const std::string& m, RemoteClient* client)
{
	OnMessage(m, &*client);
}

void RemoteServerSocket::Remove(RemoteClient* remoteClient)
{
	_lock.lock();

	std::erase(_clients, remoteClient);
	remoteClient->Stop();
	_deadClients.push_back(remoteClient);

	_lock.unlock();
}

void RemoteServerSocket::WaitForReady(int delay) const
{
	delay *= 10; 
	while (_socket == 0 && delay>0)
	{
		std::this_thread::sleep_for(std::chrono::milliseconds(100));
		delay--;
	}
}

void RemoteServerSocket::Send(const std::string& message, RemoteClient* client)
{
	if (client)
	{
		client->Send(message);
	}
	else
	{
		_lock.lock();
		const std::vector<RemoteClient*> clients(_clients);
		_lock.unlock();

		for (const auto c : clients)
		{
			if (c)
			{
				c->Send(message);
			}
		}
	}

}
