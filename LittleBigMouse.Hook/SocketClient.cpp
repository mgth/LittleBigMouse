#include "SocketClient.h"

#include <string>
#include <winsock2.h>

#include "RemoteServerSocket.h"

void SocketClient::RunThread()
{
	auto s = std::string();
	while (!_stop)
	{
		const auto n = recv(_client,_inputBuffer,sizeof(_inputBuffer),0);
		if(n<0) continue;

		auto sn = std::string(_inputBuffer,n);
		auto i = sn.find('\n');
		while(i != std::string::npos)
		{
			s += sn.substr(0,i);

			if(i+1<sn.length())
				sn = sn.substr(i+1);
			else
				sn ="";

			_server->ReceiveMessage(s);

			s = "";
			i = sn.find('\n');
		}
		s += sn;
	}
}

void SocketClient::Send(const std::string& message) const
{
	send(_client,message.c_str(),message.length(),0);
}
