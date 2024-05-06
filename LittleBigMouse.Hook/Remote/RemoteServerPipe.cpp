#include "RemoteServerPipe.h"

#include "RemoteServer.h"

#include "Strings/str.h"

bool RemoteServerPipe::StartListener()
{
    DWORD dwRead;

	const auto s = std::string(R"(\\.\pipe\lbm-daemon)");
	const auto ws = ToWString(s);
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

    while (_inputPipe != INVALID_HANDLE_VALUE && !Stopping())
	{
		LOG_TRACE("Pipe.");

	    if (ConnectNamedPipe(_inputPipe, nullptr) != false)   // wait for someone to connect to the pipe
	    {
			LOG_TRACE("Connected.");

	        while (ReadFile(_inputPipe, _inputBuffer, sizeof(_inputBuffer) - 1, &dwRead, nullptr) != FALSE)
	        {
	            /* add terminating zero */
	            _inputBuffer[dwRead] = '\0';

				LOG_TRACE(_inputBuffer);

				OnMessage(std::string(_inputBuffer),nullptr);
	        }
	    }

	    DisconnectNamedPipe(_inputPipe);
	}
	return true;
}

bool RemoteServerPipe::StartNotifier()
{
	const auto s = std::string(R"(\\.\pipe\lbm-feedback)");
	const auto ws = ToWString(s);
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
		LOG_TRACE("Pipe.");

		if (ConnectNamedPipe(_outputPipe, nullptr) != false)   // wait for someone to connect to the pipe
	    {
			LOG_TRACE("Notifier Connected.");
	    }
	}
	return true;
}


void RemoteServerPipe::Send(const std::string& message, RemoteClient* client) 
{
	if(_outputPipe != INVALID_HANDLE_VALUE)
	{
		if (message.length() > INT_MAX)
		{
				LOG_ERROR("Message too long");
				return;
		}
		const int length = message.length();

		DWORD bytes;
		WriteFile(_outputPipe,message.c_str(), length, &bytes,nullptr);
	}
}
