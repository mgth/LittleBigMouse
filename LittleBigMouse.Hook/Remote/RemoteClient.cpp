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

		if(received <= 0)
		{
			if (received == 0)
			{
				LOG_TRACE("<Client:EndConnection:" << _socket << ">");
				Stop();
				break;
			}
			if (received == SOCKET_ERROR)
			{
				auto err = WSAGetLastError();
				LOG_TRACE("<Client:SOCKET_ERROR:" << _socket << ">" << err);
				Stop();
				break;
			}

			LOG_TRACE("<Client:IllegalOutput:" << _socket << ">");
			Stop();
			break;
		}

		LOG_TRACE_1("<Client:Received:" << received << "<-" << _socket << ">");

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
			LOG_TRACE_1("<Client:MessageDone>");

			if(_listening) 
			{
				LOG_TRACE("<Client:Listening:" << _socket << ">");
				return;
			}

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

	_server->Remove(this);
}

void RemoteClient::Send(const std::string& message)
{
	LOG_TRACE("<Client:Send:" << _socket << ">" << message);
	if(Stopping()) return;

	if (message.length() > INT_MAX)
	{
			LOG_ERROR("Message too long");
			return;
	}
	const int length = message.length();

	if(_socket == 0)
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
	
	LOG_TRACE("<Client:Sent:" << _socket << ">" << result);
}

