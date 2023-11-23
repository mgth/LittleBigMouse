#pragma once
#include <string>

class RemoteClient;

class ClientMessage
{

private:
		std::string _message;
		RemoteClient* _client;
public:
		ClientMessage(std::string message, RemoteClient* client):_message(std::move(message)),_client(client){}
		[[nodiscard]] std::string Message() const {return _message;}
		[[nodiscard]] RemoteClient* Client() const {return _client;}
};
