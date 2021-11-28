; -- LittleBigMouse.iss --
#define AppVer GetFileVersion('..\bin\x64\Release\LittleBigMouse_Control.exe')
;#define AppVer '4.0-beta7'

[Setup]
AppName=Little Big Mouse
AppVersion={#AppVer}
DefaultDirName={pf}\LittleBigMouse
DefaultGroupName=Little Big Mouse
UninstallDisplayIcon={app}\LittleBigMouse_Control.exe
Compression=lzma2
SolidCompression=yes
OutputDir="."
ArchitecturesInstallIn64BitMode=x64
OutputBaseFilename=LittleBigMouse_{#AppVer}

[Files]
Source: "..\bin\x64\Release\*.exe"; DestDir: "{app}"; Check: Is64BitInstallMode
Source: "..\bin\x64\Release\*.dll"; DestDir: "{app}"; Check: Is64BitInstallMode
Source: "..\bin\x64\Release\*.xml"; DestDir: "{app}"; Check: Is64BitInstallMode; Flags: recursesubdirs

Source: "..\bin\x86\Release\*.exe"; DestDir: "{app}"; Check: not Is64BitInstallMode
Source: "..\bin\x86\Release\*.dll"; DestDir: "{app}"; Check: not Is64BitInstallMode
Source: "..\bin\x86\Release\*.xml"; DestDir: "{app}"; Check: not Is64BitInstallMode; Flags: recursesubdirs

[Icons]
Name: "{group}\Little Big Mouse"; Filename: "{app}\LittleBigMouse_Control.exe"

[Run]
Filename: {app}\LittleBigMouse_Control.exe; Description: Run Application; Flags: postinstall nowait skipifsilent runascurrentuser

[UninstallRun]
Filename: "{app}\LittleBigMouse_Daemon.exe"; Parameters: "--unschedule --exit"
