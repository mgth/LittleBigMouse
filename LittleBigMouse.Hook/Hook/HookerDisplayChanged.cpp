#include "Hooker.h"
#include "Strings/str.h"

#define MAX_LOADSTRING 100

WCHAR szWindowClass[MAX_LOADSTRING];

ATOM Hooker::RegisterClassLbm(HINSTANCE hInstance)
{
    WNDCLASSEXW wcex;

    wcex.cbSize = sizeof(WNDCLASSEX);

    wcex.style          = 0;// CS_HREDRAW | CS_VREDRAW;
    wcex.lpfnWndProc    = Hooker::DisplayChangeHandler;
    wcex.cbClsExtra     = 0;
    wcex.cbWndExtra     = 0;
    wcex.hInstance      = hInstance;
    wcex.hIcon          = nullptr; //LoadIcon(hInstance, MAKEINTRESOURCE(IDI_WINDOWSPROJECT1));
    wcex.hCursor        = nullptr; //LoadCursor(nullptr, IDC_ARROW);
    wcex.hbrBackground  = nullptr; //(HBRUSH)(COLOR_WINDOW+1);
    wcex.lpszMenuName   = nullptr; //MAKEINTRESOURCEW(IDC_WINDOWSPROJECT1);
    wcex.lpszClassName  = szWindowClass;
    wcex.hIconSm        = nullptr; //LoadIcon(wcex.hInstance, MAKEINTRESOURCE(IDI_SMALL));

    return RegisterClassExW(&wcex);
}

BOOL Hooker::InitInstance(HINSTANCE hInstance)
{
   _hInst = hInstance;

   const HWND hWnd = CreateWindowW(szWindowClass, szWindowClass, WS_OVERLAPPEDWINDOW,
      CW_USEDEFAULT, 0, CW_USEDEFAULT, 0, nullptr, nullptr, hInstance, nullptr);

   if (!hWnd)
   {
	  LOG_TRACE("<Hook:HookDisplayChange> : Failed to create window");
      return FALSE;
   }

   _hwnd = hWnd;

   //ShowWindow(hWnd, nCmdShow);
   //UpdateWindow(hWnd);

   return TRUE;
}


bool Hooker::HookDisplayChange()
{
	LOG_TRACE("<Hook:HookDisplayChange> : CreateHookWindow");

	const auto hInstance = GetModuleHandle(nullptr);

	//LoadStringW(hInstance, IDS_APP_TITLE, szWindowClass, MAX_LOADSTRING);
	wcscpy_s(szWindowClass, L"HookerDisplayChange");

    auto c = RegisterClassLbm(hInstance);

    if (!InitInstance (hInstance))
    {
        auto err = GetLastError();
        #if _DEBUG
            wchar_t buf[256];
            FormatMessageW(FORMAT_MESSAGE_FROM_SYSTEM | FORMAT_MESSAGE_IGNORE_INSERTS,
                           NULL, GetLastError(), MAKELANGID(LANG_NEUTRAL, SUBLANG_DEFAULT), 
                           buf, (sizeof(buf) / sizeof(wchar_t)), NULL);
	        LOG_TRACE("<Hook:HookDisplayChange> : " << ToString(buf));
        #endif

        return false;
    }

	return true;
}

void Hooker::UnhookDisplayChange()
{
	if (!_hwnd) return;

    DestroyWindow(_hwnd);
    _hwnd = nullptr;
}

LRESULT CALLBACK Hooker::DisplayChangeHandler(HWND hwnd, UINT msg, WPARAM wParam, LPARAM lParam)
{
    LOG_TRACE_1("<hook:DisplayChangeHandler> : " << msg);
    const auto hook = Instance();
	if (!hook) return DefWindowProc(hwnd, msg, wParam, lParam);

    switch (msg)
    {
	    case WM_DISPLAYCHANGE:
		    LOG_TRACE("<HookerDispayChanged:DisplayChanged>");
		    hook->OnDisplayChanged();
            return 0;
		case WM_SETTINGCHANGE:
			if (wParam == SPI_SETWORKAREA)
			{
				LOG_TRACE("<HookerDispayChanged:SettingChange>");
				hook->OnDisplayChanged();
				return 0;
			}
	    default: 
            return DefWindowProc(hwnd, msg, wParam, lParam);
    }
}

