; -- LittleBigMouse.iss --
#define AppVer GetVersionNumbersString('..\LittleBigMouse.Ui\LittleBigMouse.Ui.Avalonia\bin\x64\Release\net8.0\LittleBigMouse.Ui.Avalonia.exe')
;#define AppVer '5.0.0'

[Setup]
AppName=Little Big Mouse
AppVersion={#AppVer}
DefaultDirName={commonpf}\LittleBigMouse
DefaultGroupName=Little Big Mouse
UninstallDisplayIcon={app}\LittleBigMouse.Ui.Avalonia.exe
Compression=lzma2
SolidCompression=yes
OutputDir="."
ArchitecturesInstallIn64BitMode=x64
OutputBaseFilename=LittleBigMouse_{#AppVer}

[Files]
Source: "..\LittleBigMouse.Ui\LittleBigMouse.Ui.Avalonia\bin\x64\Release\net8.0\*.exe"; DestDir: "{app}"; Check: Is64BitInstallMode; Flags: recursesubdirs
Source: "..\LittleBigMouse.Daemon\bin\x64\Release\*.exe"; DestDir: "{app}"; Check: Is64BitInstallMode; Flags: recursesubdirs
Source: "..\LittleBigMouse.Ui\LittleBigMouse.Ui.Avalonia\bin\x64\Release\net8.0\*.dll"; DestDir: "{app}"; Check: Is64BitInstallMode; Flags: recursesubdirs
Source: "..\LittleBigMouse.Ui\LittleBigMouse.Ui.Avalonia\bin\x64\Release\net8.0\*.xml"; DestDir: "{app}"; Check: Is64BitInstallMode; Flags: recursesubdirs
Source: "..\LittleBigMouse.Ui\LittleBigMouse.Ui.Avalonia\bin\x64\Release\net8.0\*.json"; DestDir: "{app}"; Check: Is64BitInstallMode; Flags: recursesubdirs

;Source: "..\bin\x86\Release\net8.0\*.exe"; DestDir: "{app}"; Check: not Is64BitInstallMode
;Source: "..\bin\x86\Release\net8.0\*.dll"; DestDir: "{app}"; Check: not Is64BitInstallMode
;Source: "..\bin\x86\Release\net8.0\*.xml"; DestDir: "{app}"; Check: not Is64BitInstallMode; Flags: recursesubdirs

[Icons]
Name: "{group}\Little Big Mouse"; Filename: "{app}\LittleBigMouse.Ui.Avalonia.exe"

[Run]
Filename: {app}\LittleBigMouse.Ui.Avalonia.exe; Description: Run Application; Flags: postinstall nowait skipifsilent runascurrentuser

;[UninstallRun]
;Filename: "{cmd}"; Parameters: "/C ""taskkill /im LittleBigMouse.Ui.Avalonia.exe /f /t"
;Filename: "{cmd}"; Parameters: "/C ""taskkill /im LittleBigMouse.Hook.exe /f /t"
;Filename: "{app}\LittleBigMouse_Daemon.exe"; Parameters: "--unschedule --exit"
