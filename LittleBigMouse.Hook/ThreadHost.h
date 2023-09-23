#pragma once
#include <atomic>
#include <thread>

class ThreadHost
{
	std::thread* _thread = nullptr;

protected:
	std::atomic<bool> _stop = false;

	~ThreadHost();
	virtual void RunThread() = 0;
	virtual void DoStop();
	virtual void OnStopped();

public:
	bool Start();
	bool Stop();

	void Join() const;
};

