#include "ThreadHost.h"

#include "Daemon/LittleBigMouseDaemon.h"

ThreadHost::~ThreadHost()
{
	_mutex.lock();
		_stopping = true;

		const auto thread = _thread;
		_thread = nullptr;

		if(thread && thread->joinable() && !_joined)
		{
			thread->join();
			_joined = true;
		}
	_mutex.unlock();
	delete thread;
}

void ThreadHost::DoStop()
{
}

void ThreadHost::OnStopped()
{
}

void ThreadHost::Start()
{
	_mutex.lock();
		_stopping = false;
		_joined = false;
//		_thread = new std::thread(&ThreadHost::Run, this);
		_thread = new std::thread([this] { Run(); });
	_mutex.unlock();
}

void ThreadHost::RunThread()
{
}

void ThreadHost::Run()
{
	RunThread();
	OnStopped();
}

void ThreadHost::Stop()
{
	_mutex.lock();
	_stopping = true;
	_mutex.unlock();
	DoStop();
}

void ThreadHost::Join() 
{
	_mutex.lock();
		const auto thread = _thread;

		if(thread && thread->joinable() && !_joined) 
		{
			thread->join();
			_joined = true;
		}
	_mutex.unlock();
}
