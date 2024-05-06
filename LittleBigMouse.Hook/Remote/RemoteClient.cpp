#include "Framework.h"

#include "RemoteClient.h"
#include "RemoteServerSocket.h"

#include <string>

void RemoteClient::RunThread()
{
	auto message = std::string();

	LOG_TRACE("<Client:Start:" << _socket << ">");
	auto stopping = false;
	while (!stopping)
	{
		const auto received = recv(_socket,_inputBuffer,sizeof(_inputBuffer),0);
		if (received <= 0)
		{
			Stop();
			break;
		}

		LOG_TRACE("<Client:Received:" << received << "<-" << _socket << ">");

		auto toParse = std::string(_inputBuffer,received);
		auto eol = toParse.find('\n');
		while(eol != std::string::npos)
		{
			message += toParse.substr(0,eol);

			eol++;
			if(eol<toParse.length())
				toParse = toParse.substr(eol);
			else
				toParse ="";

			LOG_TRACE("<Client:MessageReceived:" << message << ">" );
			if (_server)
				_server->ReceiveMessage(message, this);
			LOG_TRACE("<Client:MessageDone>");

			message = "";
			eol = toParse.find('\n');
		}
		message += toParse;
		stopping = Stopping();
	}

	LOG_TRACE("<Client:Stopped:" << _socket << ">");

	if(_socket)
	{
		closesocket(_socket);
		_socket = 0;
	}
	_server->Remove(this);
}

void RemoteClient::DoStop()
{
	ThreadHost::DoStop();
	if(_socket)
		shutdown(_socket,2);
	if(_socket)
		closesocket(_socket);
}

void RemoteClient::Send(const std::string& message)
{
	if(Stopping()) return;

	if (message.length() > INT_MAX)
	{
			LOG_ERROR("Message too long");
			return;
	}
	const int length = message.length();

	if(!_socket)
	{
		LOG_ERROR("No socket");
		Stop();
		return;
	}

	const auto result = send(_socket,message.c_str(),length,0);
	if(result == SOCKET_ERROR)
	{
		Stop();
		if(_socket)
		{
			closesocket(_socket);
			_socket = 0;
		}
	}
	
	LOG_TRACE("<Client:Sent:" << result << "->" << _socket << ">" << message );
}
