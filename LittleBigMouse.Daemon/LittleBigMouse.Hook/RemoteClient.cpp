#include <string>
#include "RemoteClient.h"
#include "RemoteServerSocket.h"

void RemoteClient::RunThread()
{
	auto s = std::string();

	while (!_stop)
	{
		const auto n = recv(_client,_inputBuffer,sizeof(_inputBuffer),0);
		if(n == 0)
		{
			_stop = true;
			break;
		}

		auto sn = std::string(_inputBuffer,n);
		auto i = sn.find('\n');
		while(i != std::string::npos)
		{
			s += sn.substr(0,i);

			if(i+1<sn.length())
				sn = sn.substr(i+1);
			else
				sn ="";

			_server->ReceiveMessage(s, this);

			s = "";
			i = sn.find('\n');
		}
		s += sn;
	}

	_server->Remove(this);
}

void RemoteClient::Send(const std::string& message)
{
	const auto result = send(_client,message.c_str(),message.length(),0);
	if(result == SOCKET_ERROR)
	{
		_stop = true;
	}
}
