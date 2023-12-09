#include "RemoteServer.h"
#include "LittleBigMouseDaemon.h"
//#include <cstddef>
#include "RemoteServerPipe.h"

#include <Windows.h>
#include <iostream>

#include "ClientMessage.h"
#include "str.h"

bool RemoteServerPipe::StartListener()
{
    DWORD dwRead;

	const auto s = std::string(R"(\\.\pipe\lbm-daemon)");
	const auto ws = to_wstring(s);
	_inputPipe = CreateNamedPipe(
        ws.c_str(), //L"\\\\.\\pipe\\lbm-daemon-beta",// name of the pipe
        PIPE_ACCESS_INBOUND, // 1-way pipe -- receive
        PIPE_TYPE_MESSAGE, // send data as a byte stream
        1, // only allow 1 instance of this pipe
        0, // no outbound buffer
        sizeof(_inputBuffer), // inbound buffer
        0, // use default wait time
        nullptr        // use default security attributes
    );

    while (_inputPipe != INVALID_HANDLE_VALUE && !_stop)
	{
		#if defined(_DEBUG)
		std::cout << "Pipe.\n";
		#endif
	    if (ConnectNamedPipe(_inputPipe, nullptr) != false)   // wait for someone to connect to the pipe
	    {
			#if defined(_DEBUG)
			std::cout << "Connected.\n";
			#endif

	        while (ReadFile(_inputPipe, _inputBuffer, sizeof(_inputBuffer) - 1, &dwRead, nullptr) != FALSE)
	        {
	            /* add terminating zero */
	            _inputBuffer[dwRead] = '\0';

				#if defined(_DEBUG)
				std::cout << _inputBuffer;
				#endif

				OnMessage.fire(std::string(_inputBuffer),nullptr);
	        }
	    }

	    DisconnectNamedPipe(_inputPipe);
	}
	return true;
}

bool RemoteServerPipe::StartNotifier()
{
	const auto s = std::string(R"(\\.\pipe\lbm-feedback)");
	const auto ws = to_wstring(s);
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
		#if defined(_DEBUG)
		std::cout << "Pipe.\n";
		#endif
	    if (ConnectNamedPipe(_outputPipe, nullptr) != false)   // wait for someone to connect to the pipe
	    {
			#if defined(_DEBUG)
			std::cout << "Notifier Connected.\n";
			#endif
	    }
	}
	return true;
}


void RemoteServerPipe::Send(const std::string& message, RemoteClient* client) 
{
	if(_outputPipe != INVALID_HANDLE_VALUE)
	{
		DWORD bytes;
		WriteFile(_outputPipe,message.c_str(),message.size(),&bytes,nullptr);
	}
}
