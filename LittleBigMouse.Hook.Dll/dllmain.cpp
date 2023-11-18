#include "pch.h"

#include <fstream>
#include <iostream>
#include <shlobj_core.h>
#include <Shlwapi.h>

#include "Point.h"
#include "HookMouseEventArg.h"

#include <iosfwd>
#include <string>
#include <MouseEngine.h>

static MouseEngine *engine = nullptr;

void LoadFromFile(const std::wstring& path)
{
    PWSTR szPath;

    if (SUCCEEDED(SHGetKnownFolderPath(FOLDERID_ProgramData, 0, nullptr, &szPath)))
    {
	    std::ifstream startup;

        PathAppend(szPath, path.c_str());
	    startup.open(szPath, std::ios::in);

	    std::string buffer;
		std::string line;
		while(startup){
			std::getline(startup, line);
			tinyxml2::XMLDocument doc;
			doc.Parse(line.c_str());

		    engine->Layout.Load(doc.RootElement());
		}

	    startup.close();
    }

}
void LoadFromCurrentFile()
{
	LoadFromFile(TEXT("\\Mgth\\LittleBigMouse\\Layout.xml"));
}


BOOL APIENTRY DllMain( HMODULE hModule,
                       DWORD  ul_reason_for_call,
                       LPVOID lpReserved
)
{
    switch (ul_reason_for_call)
    {
	    case DLL_PROCESS_ATTACH:
	    case DLL_THREAD_ATTACH:
	    case DLL_THREAD_DETACH:
	    case DLL_PROCESS_DETACH:
        break;
		default: ;
    }

    return TRUE;
}

extern "C" __declspec(dllexport) int WindowCallback(int code, WPARAM wParam, LPARAM lParam) {

	if (code == HCBT_SETFOCUS)
	{
        FILE *file;

		fopen_s(&file, "L:function.txt", "a+");

        fprintf(file, "HCBT_SETFOCUS : ");
        fprintf(file, "\n");

		fclose(file);

		std::cout << "HCBT_SETFOCUS" << wParam << std::endl;
	}

	return(CallNextHookEx(NULL, code, wParam, lParam));

}

static LRESULT __stdcall MouseCallback(const int nCode, const WPARAM wParam, const LPARAM lParam)
{
	static auto previousLocation = geo::Point<long>();
	const auto pMouse = reinterpret_cast<MSLLHOOKSTRUCT*>(lParam);

	if (nCode >= 0 && lParam != NULL && (wParam & WM_MOUSEMOVE) != 0)
	{
		const auto location = geo::Point<long>(pMouse->pt.x,pMouse->pt.y);

		if ( previousLocation != location)
		{
			previousLocation = location;

			auto p = MouseEventArg(location);

			engine->OnMouseMove(p);

			if (p.Handled) return 1;
			//if (p.Handled) return -1;
		}
	}

    return CallNextHookEx(NULL, nCode, wParam, lParam);
}




