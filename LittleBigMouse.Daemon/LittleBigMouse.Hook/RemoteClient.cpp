#include <string>
#include "RemoteClient.h"
#include "RemoteServerSocket.h"
#include <iostream>

void RemoteClient::RunThread()
{
	auto s = std::string();

	#if defined(_DEBUG)
	std::cout << "<Client:Start:" << _socket << ">\n";
	#endif

	while (!_stop)
	{
		const auto n = recv(_socket,_inputBuffer,sizeof(_inputBuffer),0);
		if (n <= 0)
		{
			_stop = true;
			break;
		}

		#if defined(_DEBUG)
		std::cout << "<Client:Received:" << n << "<-" << _socket << ">\n";
		#endif

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

	#if defined(_DEBUG)
	std::cout << "<Client:Stopped:" << _socket << ">\n";
	#endif

	if(_socket)
	{
		closesocket(_socket);
		_socket = 0;
	}
	_server->Remove(this);
}

void RemoteClient::Send(const std::string& message)
{
	const auto result = send(_socket,message.c_str(),message.length(),0);
	if(result == SOCKET_ERROR)
	{
		_stop = true;
		if(_socket)
		{
			closesocket(_socket);
			_socket = 0;
		}
	}
	#if defined(_DEBUG)
	std::cout << "<Client:Sent:" << result << "->" << _socket << ">" << message << "\n";
	#endif
}
