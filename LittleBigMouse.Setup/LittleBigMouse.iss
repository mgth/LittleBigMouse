; -- LittleBigMouse.iss --
; AppVer is normally passed by CI from the git tag (ISCC /DAppVer=5.3.0-beta.2)
; so betas are distinguishable in Programs & Features and in the installer name.
; Falls back to the built exe's numeric FileVersion for a manual local compile.
#ifndef AppVer
  #define AppVer GetVersionNumbersString('..\LittleBigMouse.Ui\LittleBigMouse.Ui.Avalonia\bin\x64\Release\net10.0\LittleBigMouse.Ui.Avalonia.exe')
#endif

; Which hook daemon was built (CI passes /DHookImpl, rust being the default). The
; hook exe is staged into the UI output either way and packaged by the UI *.exe
; line below; the extra C++ bin source is only needed for the C++ opt-out build.
#ifndef HookImpl
  #define HookImpl "rust"
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
var
  DotNetDownloadPage: TDownloadWizardPage;
  DotNetInstallAttempted: Boolean;

{ The app ships framework-dependent: it needs the .NET 10 Runtime (x64), which a
  freshly (re)installed Windows does not have (#510) — and a missing runtime means
  the exe just shows an apphost dialog, or nothing at all. Detect the standard
  install location; when missing, pull the runtime from the official aka.ms
  channel link and install it silently before the files are copied. If the
  download or the install fails, the app files are still installed and the user
  is pointed at the manual download page. }
function DotNet10RuntimeInstalled: Boolean;
var
  FindRec: TFindRec;
begin
  { 32-bit Windows can't run the x64 build anyway; don't try to fix it here. }
  if not IsWin64 then begin
    Result := True;
    Exit;
  end;
  Result := False;
  if FindFirst(ExpandConstant('{commonpf64}\dotnet\shared\Microsoft.NETCore.App\10.*'), FindRec) then begin
    try
      repeat
        if FindRec.Attributes and FILE_ATTRIBUTE_DIRECTORY <> 0 then begin
          Result := True;
          Break;
        end;
      until not FindNext(FindRec);
    finally
      FindClose(FindRec);
    end;
  end;
end;

function RunDotNetRuntimeInstaller: Boolean;
var
  ResultCode: Integer;
begin
  { 3010 = success, reboot required. }
  Result := Exec(ExpandConstant('{tmp}\dotnet-runtime-win-x64.exe'),
                 '/install /quiet /norestart', '', SW_SHOW, ewWaitUntilTerminated, ResultCode)
            and ((ResultCode = 0) or (ResultCode = 3010));
  if not Result then
    Log('dotnet runtime installer exit code: ' + IntToStr(ResultCode));
end;

function InstallDotNetRuntime: Boolean;
begin
  Result := False;
  DotNetInstallAttempted := True;
  DotNetDownloadPage.Clear;
  DotNetDownloadPage.Add('https://aka.ms/dotnet/10.0/dotnet-runtime-win-x64.exe',
                         'dotnet-runtime-win-x64.exe', '');
  DotNetDownloadPage.Show;
  try
    try
      DotNetDownloadPage.Download;
    except
      if DotNetDownloadPage.AbortedByUser then
        Log('.NET runtime download aborted by user.')
      else
        Log('.NET runtime download failed: ' + GetExceptionMessage);
      Exit;
    end;
    Result := RunDotNetRuntimeInstaller;
  finally
    DotNetDownloadPage.Hide;
  end;
end;

procedure InitializeWizard;
begin
  DotNetDownloadPage := CreateDownloadPage(SetupMessage(msgWizardPreparing),
                                           SetupMessage(msgPreparingDesc), nil);
end;

function NextButtonClick(CurPageID: Integer): Boolean;
begin
  Result := True;
  if (CurPageID = wpReady) and not DotNet10RuntimeInstalled then begin
    if not InstallDotNetRuntime then
      SuppressibleMsgBox(
        'Setup could not install the .NET 10 Runtime (x64), which Little Big Mouse requires.'#13#10 +
        'The application will be installed but will not start until you install the runtime from:'#13#10 +
        'https://dotnet.microsoft.com/download/dotnet/10.0',
        mbError, MB_OK, IDOK);
  end;
end;

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
  { Silent installs never reach NextButtonClick: give the runtime one headless
    attempt here (no progress UI). Skipped when the interactive path already
    tried, whatever its outcome — no point failing twice. }
  if not DotNetInstallAttempted and not DotNet10RuntimeInstalled then begin
    DotNetInstallAttempted := True;
    try
      DownloadTemporaryFile('https://aka.ms/dotnet/10.0/dotnet-runtime-win-x64.exe',
                            'dotnet-runtime-win-x64.exe', '', nil);
      RunDotNetRuntimeInstaller;
    except
      Log('.NET runtime download failed: ' + GetExceptionMessage);
    end;
  end;
  Result := '';
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usUninstall then
    StopLittleBigMouse;
end;

