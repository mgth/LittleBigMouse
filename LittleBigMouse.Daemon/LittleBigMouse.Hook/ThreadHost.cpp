#include "ThreadHost.h"

#include <iostream>

#include "LittleBigMouseDaemon.h"

ThreadHost::~ThreadHost()
{
	while(_thread)
		Stop();
}

void ThreadHost::DoStop()
{
	_stop = true;
}

void ThreadHost::OnStopped()
{
	_stop = false;
}

bool ThreadHost::Start()
{
	_thread = new std::thread(&ThreadHost::RunThread, this);
	return true;
}

bool ThreadHost::Stop()
{
	if (_thread && _thread->joinable())
	{
		DoStop();

		_thread->join();
		_thread=nullptr;

		OnStopped();
		return true;
	}
	return false;
}

void ThreadHost::Join() const
{
	if(_thread && _thread->joinable()) 
		_thread->join();
}
