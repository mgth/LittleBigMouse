#pragma once
#include <atomic>
#include <thread>
#include <mutex>

class ThreadHost
{
	std::thread* _thread = nullptr;

protected:
	std::atomic<bool> _stop = false;
	std::atomic<bool> _joined = false;

	~ThreadHost();

	virtual void RunThread() = 0;
	virtual void DoStop();
	virtual void OnStopped();
	void Run();

public:
	void Start();
	void Stop();

	void Join();
};

