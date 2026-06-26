#include "Logger.h"

#if _DEBUG && DEBUG_LOG
#include <windows.h>
#include <string>
std::mutex Logger::Lock;

#ifdef LOGTOFILE
static std::string LogFilePath()
{
	char base[MAX_PATH] = { 0 };
	const DWORD n = GetEnvironmentVariableA("ProgramData", base, MAX_PATH);
	if (n == 0 || n >= MAX_PATH)
		return "lbm-hook.log";

	const std::string dir = std::string(base) + "\\Mgth\\LittleBigMouse";
	CreateDirectoryA((std::string(base) + "\\Mgth").c_str(), nullptr);
	CreateDirectoryA(dir.c_str(), nullptr);
	return dir + "\\hook.log";
}
std::ofstream Logger::Out = std::ofstream(LogFilePath(), std::ios::app);
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

