#include "RemoteServer.h"
#include "LittleBigMouseDaemon.h"
//#include <cstddef>
#include <Windows.h>
#include <iostream>

void RemoteServer::StartListener(const std::string name)
{
	char outputbuffer[OUTPUTBUFFERSIZE];
    char inputBuffer[INPUTBUFFERSIZE];
    DWORD dwRead;

	const auto s = R"(\\.\pipe\)" + name;
	const auto ws = std::wstring(s.begin(), s.end());
	_inputPipe = CreateNamedPipe(
        ws.c_str(), //L"\\\\.\\pipe\\lbm-daemon-beta",// name of the pipe
        PIPE_ACCESS_INBOUND, // 1-way pipe -- receive
        PIPE_TYPE_MESSAGE, // send data as a byte stream
        1, // only allow 1 instance of this pipe
        0, // no outbound buffer
        sizeof(inputBuffer), // inbound buffer
        0, // use default wait time
        nullptr        // use default security attributes
    );

    while (_inputPipe != INVALID_HANDLE_VALUE)
	{
		std::cout << "Pipe.\n";
	    if (ConnectNamedPipe(_inputPipe, nullptr) != false)   // wait for someone to connect to the pipe
	    {
			std::cout << "Connected.\n";

	        while (ReadFile(_inputPipe, inputBuffer, sizeof(inputBuffer) - 1, &dwRead, nullptr) != FALSE)
	        {
	            /* add terminating zero */
	            inputBuffer[dwRead] = '\0';

				std::cout << inputBuffer;

	            Daemon->ReceiveMessage(std::string(inputBuffer));
	        }
	    }

	    DisconnectNamedPipe(_inputPipe);
	}
}

void RemoteServer::StartNotifier(const std::string name)
{
	const auto s = R"(\\.\pipe\)" + name;
	const auto ws = std::wstring(s.begin(), s.end());
	_outputPipe = CreateNamedPipe(
        ws.c_str(), //L"\\\\.\\pipe\\lbm-daemon-beta",// name of the pipe
        PIPE_ACCESS_OUTBOUND, // 1-way pipe -- receive
        PIPE_TYPE_MESSAGE, // send data as a byte stream
        1, // only allow 1 instance of this pipe
        0, // no outbound buffer
        0, // inbound buffer
        0, // use default wait time
        nullptr        // use default security attributes
    );

    while (_inputPipe != INVALID_HANDLE_VALUE)
	{
		std::cout << "Pipe.\n";
	    if (ConnectNamedPipe(_outputPipe, nullptr) != false)   // wait for someone to connect to the pipe
	    {
			std::cout << "Notifier Connected.\n";

	    }
	}
}


void RemoteServer::Send(const std::string& message) const
{
	if(_outputPipe != INVALID_HANDLE_VALUE)
	{
		DWORD bytes;
		WriteFile(_outputPipe,message.c_str(),message.size(),&bytes,nullptr);
	}
}
