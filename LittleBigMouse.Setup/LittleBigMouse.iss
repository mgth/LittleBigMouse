; -- LittleBigMouse.iss --
; AppVer is normally passed by CI from the git tag (ISCC /DAppVer=5.3.0-beta.2)
; so betas are distinguishable in Programs & Features and in the installer name.
; Falls back to the built exe's numeric FileVersion for a manual local compile.
#ifndef AppVer
  #define AppVer GetVersionNumbersString('..\artifacts\publish\win-x64\LittleBigMouse.Ui.Avalonia.exe')
#endif
#ifndef PublishDir
  #define PublishDir "..\artifacts\publish\win-x64"
#endif

[Setup]
AppId={{C170D4ED-CDCC-4383-8907-B85461E643FF}
AppName=Little Big Mouse
AppVersion={#AppVer}
AppPublisher=Little Big Mouse contributors
AppPublisherURL=https://github.com/mgth/LittleBigMouse
AppSupportURL=https://github.com/mgth/LittleBigMouse/issues
AppUpdatesURL=https://github.com/mgth/LittleBigMouse/releases
DefaultDirName={commonpf}\LittleBigMouse
DefaultGroupName=Little Big Mouse
UninstallDisplayIcon={app}\LittleBigMouse.Ui.Avalonia.exe
; Give the setup .exe the LBM icon (the NSIS installer used to, the Inno one didn't).
SetupIconFile=..\LittleBigMouse.Ui\LittleBigMouse.Ui.Avalonia\Assets\lbm-logo.ico
Compression=lzma2
SolidCompression=yes
OutputDir="."
ArchitecturesInstallIn64BitMode=x64compatible
ArchitecturesAllowed=x64compatible
OutputBaseFilename=LittleBigMouse_{#AppVer}
; Force-close the running app instead of asking the user: the Restart Manager's
; polite WM_CLOSE is treated as "minimize to tray" by the UI and ignored by the
; windowless hook, so the default (CloseApplications=yes) leaves files locked.
; A hard taskkill in [Code] backs this up (see StopLittleBigMouse).
CloseApplications=force
RestartApplications=no

[Files]
Source: "{#PublishDir}\*"; DestDir: "{app}"; Check: Is64BitInstallMode; Flags: recursesubdirs createallsubdirs

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
var
  RemoveUserData: Boolean;

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

function HasParameter(const Expected: String): Boolean;
var
  Index: Integer;
begin
  Result := False;
  for Index := 1 to ParamCount do
    if CompareText(ParamStr(Index), Expected) = 0 then begin
      Result := True;
      Exit;
    end;
end;

function InitializeUninstall: Boolean;
begin
  RemoveUserData := HasParameter('/REMOVEUSERDATA');
  if not RemoveUserData and not UninstallSilent then
    RemoveUserData := MsgBox(
      'Also remove Little Big Mouse settings, recovery files, and smart-TV pairing data?'#13#10#13#10 +
      'Choose No to preserve them for a reinstall.',
      mbConfirmation, MB_YESNO) = IDYES;
  Result := True;
end;

procedure RemoveScheduledTasks;
var
  ResultCode: Integer;
  Command: String;
begin
  Command := '-NoProfile -NonInteractive -ExecutionPolicy Bypass -Command "' +
    'Get-ScheduledTask -TaskName ''LittleBigMouse_*'' -ErrorAction SilentlyContinue | ' +
    'Where-Object { $_.Actions.Execute -eq ''' +
    ExpandConstant('{app}\LittleBigMouse.Ui.Avalonia.exe') + ''' } | ' +
    'Unregister-ScheduledTask -Confirm:$false"';
  if not Exec(ExpandConstant('{sys}\WindowsPowerShell\v1.0\powershell.exe'),
              Command, '', SW_HIDE, ewWaitUntilTerminated, ResultCode) then
    Log('Could not start scheduled-task cleanup.');
end;

procedure RemoveSettings;
begin
  RegDeleteKeyIncludingSubkeys(HKCU, 'SOFTWARE\Mgth\LittleBigMouse');
  DelTree(ExpandConstant('{localappdata}\Mgth\LittleBigMouse'), True, True, True);
  DelTree(ExpandConstant('{userappdata}\LittleBigMouse'), True, True, True);
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usUninstall then
  begin
    StopLittleBigMouse;
    RemoveScheduledTasks;
  end;
  if (CurUninstallStep = usPostUninstall) and RemoveUserData then
    RemoveSettings;
end;

