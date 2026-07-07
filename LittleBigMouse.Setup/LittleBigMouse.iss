; -- LittleBigMouse.iss --
; AppVer is normally passed by CI from the git tag (ISCC /DAppVer=5.3.0-beta.2)
; so betas are distinguishable in Programs & Features and in the installer name.
; Falls back to the built exe's numeric FileVersion for a manual local compile.
#ifndef AppVer
  #define AppVer GetVersionNumbersString('..\LittleBigMouse.Ui\LittleBigMouse.Ui.Avalonia\bin\x64\Release\net10.0\LittleBigMouse.Ui.Avalonia.exe')
#endif

; Which hook daemon was built (CI passes /DHookImpl=rust for the Rust port). The
; hook exe is staged into the UI output either way and packaged by the UI *.exe
; line below; the extra C++ bin source is only needed for the C++ build.
#ifndef HookImpl
  #define HookImpl "cpp"
#endif

[Setup]
AppId={{C170D4ED-CDCC-4383-8907-B85461E643FF}
AppName=Little Big Mouse
AppVersion={#AppVer}
DefaultDirName={commonpf}\LittleBigMouse
DefaultGroupName=Little Big Mouse
UninstallDisplayIcon={app}\LittleBigMouse.Ui.Avalonia.exe
; Give the setup .exe the LBM icon (the NSIS installer used to, the Inno one didn't).
SetupIconFile=..\LittleBigMouse.Ui\LittleBigMouse.Ui.Avalonia\Assets\lbm-logo.ico
Compression=lzma2
SolidCompression=yes
OutputDir="."
ArchitecturesInstallIn64BitMode=x64
OutputBaseFilename=LittleBigMouse_{#AppVer}
; Force-close the running app instead of asking the user: the Restart Manager's
; polite WM_CLOSE is treated as "minimize to tray" by the UI and ignored by the
; windowless hook, so the default (CloseApplications=yes) leaves files locked.
; A hard taskkill in [Code] backs this up (see StopLittleBigMouse).
CloseApplications=force
RestartApplications=no

[Files]
Source: "..\LittleBigMouse.Ui\LittleBigMouse.Ui.Avalonia\bin\x64\Release\net10.0\*.exe"; DestDir: "{app}"; Check: Is64BitInstallMode; Flags: recursesubdirs
#if HookImpl == "cpp"
Source: "..\LittleBigMouse.Hook\bin\x64\Release\*.exe"; DestDir: "{app}"; Check: Is64BitInstallMode; Flags: recursesubdirs
#endif
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

[Code]
procedure KillImage(const ExeName: String);
var
  ResultCode: Integer;
begin
  Exec(ExpandConstant('{sys}\taskkill.exe'), '/F /IM ' + ExeName, '',
       SW_HIDE, ewWaitUntilTerminated, ResultCode);
end;

procedure StopLittleBigMouse;
begin
  { Hard-kill the running app (UI loader, UI, and the elevated hook) so their files
    unlock before we copy/remove them. Kill by EXACT image name only: a wildcard like
    "LittleBigMouse*" would also match the setup EXE itself (LittleBigMouse_<ver>.exe)
    and the installer would terminate itself mid-install. The installer runs elevated,
    so it can end the elevated hook. Missing processes just make taskkill a no-op. }
  KillImage('LittleBigMouse.Ui.Loader.exe');
  KillImage('LittleBigMouse.Ui.Avalonia.exe');
  KillImage('LittleBigMouse.Hook.exe');
  KillImage('LittleBigMouse_Daemon.exe');
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  StopLittleBigMouse;
  Result := '';
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usUninstall then
    StopLittleBigMouse;
end;

