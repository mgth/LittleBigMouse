#include <iostream>

#include "ThreadHost.h"
#include "LittleBigMouseDaemon.h"

ThreadHost::~ThreadHost()
{
	if(_thread && _thread->joinable() && !_joined.load())
	{
		DoStop();
		_thread->join();
	}
	delete _thread;
}

void ThreadHost::DoStop()
{
	_stop.store(true);
}

void ThreadHost::OnStopped()
{
}

void ThreadHost::Start()
{
	_stop.store(false);
	_thread = new std::thread(&ThreadHost::Run, this);
	_joined.store(false);
}

void ThreadHost::Run()
{
	RunThread();
	OnStopped();
}

void ThreadHost::Stop()
{
	DoStop();
}

void ThreadHost::Join() 
{
	if(_thread && _thread->joinable() && !_joined.load()) 
	{
		_thread->join();
		_joined.store(true);
	}
}
