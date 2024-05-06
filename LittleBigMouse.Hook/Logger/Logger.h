#pragma once

#if _DEBUG && DEBUG_LOG 

	#include <iostream>
	#include <mutex>
	#include <chrono>
	#include <type_traits>
	#include <format>

	class Logger
	{
		public:
		// lock mutex :
			static std::mutex Lock;
	#ifdef LOGTOFILE
			static std::ofstream Out;
	#else
			static std::ostream Out;
	#endif // LOGTOFILE

			static void Flush();

			static void Close();
	};

	#define TIME_NOW (std::format("{0:%F_%T}", std::chrono::system_clock::now()))
	#if LOGTOFILE
		#include <fstream>
	#endif

	#define LOG(x) {Logger::Lock.lock(); Logger::Out << x << '\n'; Logger::Flush(); Logger::Lock.unlock(); }
//		#define LOG(x) cout << x << '\n'
//	#endif
	#define LOG_LOCATION LOG("[" << __FILE__ << "][" << __FUNCTION__ << "][Line " << __LINE__ << "] ")
	#define LOG_TRACE(x) LOG (TIME_NOW << " : " << x)
	#define LOG_TRACE_1(x) NULL

	#if DEBUG_LEVEL > 1
		#define LOG_TRACE_1(x) LOG_TRACE(x);
	#else
		#define LOG_TRACE_1(x) NULL
	#endif

	#define LOG_ERROR(x) LOG_LOCATION; LOG(TIME_NOW << " ERROR : " << x )
	#define LOG_INFO(x) LOG_LOCATION; LOG(TIME_NOW << " INFO : " << x )
	#define LOG_DEBUG(x) LOG_LOCATION; LOG (TIME_NOW << " DEBG : " << x )

	#define LOG_CLOSE Logger::Close()
#else
	#define LOG_ERROR(x) {}
	#define LOG_INFO(x) {}
	#define LOG_DEBUG(x) {}
	#define LOG_TRACE(x) {}
	#define LOG_TRACE_1(x) {}
	#define LOG_CLOSE {}
#endif

