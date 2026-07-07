; -- LittleBigMouse.iss --
; AppVer is normally passed by CI from the git tag (ISCC /DAppVer=5.3.0-beta.2)
; so betas are distinguishable in Programs & Features and in the installer name.
; Falls back to the built exe's numeric FileVersion for a manual local compile.
#ifndef AppVer
  #define AppVer GetVersionNumbersString('..\LittleBigMouse.Ui\LittleBigMouse.Ui.Avalonia\bin\x64\Release\net10.0\LittleBigMouse.Ui.Avalonia.exe')
#endif

[Setup]
AppId={{C170D4ED-CDCC-4383-8907-B85461E643FF}
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
Source: "..\LittleBigMouse.Ui\LittleBigMouse.Ui.Avalonia\bin\x64\Release\net10.0\*.exe"; DestDir: "{app}"; Check: Is64BitInstallMode; Flags: recursesubdirs
Source: "..\LittleBigMouse.Hook\bin\x64\Release\*.exe"; DestDir: "{app}"; Check: Is64BitInstallMode; Flags: recursesubdirs
Source: "..\LittleBigMouse.Ui\LittleBigMouse.Ui.Avalonia\bin\x64\Release\net10.0\*.dll"; DestDir: "{app}"; Check: Is64BitInstallMode; Flags: recursesubdirs
Source: "..\LittleBigMouse.Ui\LittleBigMouse.Ui.Avalonia\bin\x64\Release\net10.0\*.xml"; DestDir: "{app}"; Check: Is64BitInstallMode; Flags: recursesubdirs
Source: "..\LittleBigMouse.Ui\LittleBigMouse.Ui.Avalonia\bin\x64\Release\net10.0\*.json"; DestDir: "{app}"; Check: Is64BitInstallMode; Flags: recursesubdirs

;Source: "..\bin\x86\Release\net10.0\*.exe"; DestDir: "{app}"; Check: not Is64BitInstallMode
;Source: "..\bin\x86\Release\net10.0\*.dll"; DestDir: "{app}"; Check: not Is64BitInstallMode
;Source: "..\bin\x86\Release\net10.0\*.xml"; DestDir: "{app}"; Check: not Is64BitInstallMode; Flags: recursesubdirs

[InstallDelete]
; Old NSIS install (<= 5.2.3) lived in the same folder; drop its leftover uninstaller.
Type: files; Name: "{app}\Uninstall.exe"

[Registry]
; Remove the stale "LittleBigMouse" (NSIS, <= 5.2.3) Programs & Features entry so it
; no longer shows up next to the Inno-installed one. Cover both registry views.
Root: HKLM64; Subkey: "Software\Microsoft\Windows\CurrentVersion\Uninstall\LittleBigMouse"; Flags: deletekey; Check: IsWin64
Root: HKLM32; Subkey: "Software\Microsoft\Windows\CurrentVersion\Uninstall\LittleBigMouse"; Flags: deletekey

[Icons]
Name: "{group}\Little Big Mouse"; Filename: "{app}\LittleBigMouse.Ui.Avalonia.exe"

[Run]
Filename: {app}\LittleBigMouse.Ui.Avalonia.exe; Description: Run Application; Flags: postinstall nowait skipifsilent runascurrentuser

;[UninstallRun]
;Filename: "{cmd}"; Parameters: "/C ""taskkill /im LittleBigMouse.Ui.Avalonia.exe /f /t"
;Filename: "{cmd}"; Parameters: "/C ""taskkill /im LittleBigMouse.Hook.exe /f /t"
;Filename: "{app}\LittleBigMouse_Daemon.exe"; Parameters: "--unschedule --exit"

