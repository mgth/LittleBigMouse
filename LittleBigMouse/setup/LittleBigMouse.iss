; -- LittleBigMouse.iss --
[Setup]
AppName=Little Big Mouse
AppVersion=1.0
DefaultDirName={pf}\LittleBigMouse
DefaultGroupName=Little Big Mouse
UninstallDisplayIcon={app}\LittleBigMouse.exe
Compression=lzma2
SolidCompression=yes
OutputDir=userdocs:Inno Setup Examples Output
ArchitecturesInstallIn64BitMode=x64

[Files]
Source: "..\bin\x64\Release\LittleBigMouse.exe"; DestDir: "{app}"; Check: Is64BitInstallMode
Source: "..\bin\x64\Release\Microsoft.Win32.TaskScheduler.dll"; DestDir: "{app}"; Check: Is64BitInstallMode
Source: "..\bin\x64\Release\MouseKeyboardActivityMonitor.dll"; DestDir: "{app}"; Check: Is64BitInstallMode
Source: "..\bin\x86\Release\LittleBigMouse.exe"; DestDir: "{app}"; Check: not Is64BitInstallMode
Source: "..\bin\x86\Release\Microsoft.Win32.TaskScheduler.dll"; DestDir: "{app}"; Check: not Is64BitInstallMode
Source: "..\bin\x86\Release\MouseKeyboardActivityMonitor.dll"; DestDir: "{app}"; Check: not Is64BitInstallMode

[Icons]
Name: "{group}\Little Big Mouse"; Filename: "{app}\LittleBigMouse.exe"

[Run]
Filename: {app}\LittleBigMouse.exe; Description: Run Application; Flags: postinstall nowait skipifsilent runascurrentuser 