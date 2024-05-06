#pragma once
#include "Framework.h"

#include <thread>
#include <mutex>

class ThreadHost
{
private:
	std::mutex _mutex = std::mutex();
	std::thread* _thread = nullptr;
	bool _stopping = false;
	bool _joined = false;
	void Run();

protected:

	~ThreadHost();
    ThreadHost(ThreadHost&& other) noexcept : _thread(_STD exchange(other._thread, {})) {}
    ThreadHost& operator=(ThreadHost&& other) noexcept {
        if (_thread->joinable()) {
            _STD terminate(); // per N4950 [thread.thread.assign]/1
        }

        _thread = _STD exchange(other._thread, {});
        return *this;
    }


	virtual void RunThread();
	virtual void DoStop();
	virtual void OnStopped();
	[[nodiscard]] bool Stopping() const { return _stopping; }
	void Lock() { _mutex.lock(); }
	void Unlock() { _mutex.unlock(); }

public:
	ThreadHost(const ThreadHost&)            = delete;
    ThreadHost& operator=(const ThreadHost&) = delete;
	ThreadHost() = default;

	void Start();
	void Stop();

	void Join();
};

