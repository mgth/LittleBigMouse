#include "Logger.h"

#if _DEBUG && DEBUG_LOG
std::mutex Logger::Lock;

#ifdef LOGTOFILE
std::ofstream Logger::Out = std::ofstream("L:\\log.txt", std::ios::app);
#else
std::ostream Logger::Out = std::cout;
#endif // LOGTOFILE
void Logger::Flush() { Out.flush(); }

void Logger::Close()
{
#ifdef LOGTOFILE
	Out.close();
#endif
}

#endif 

