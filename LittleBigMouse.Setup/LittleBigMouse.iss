; -- LittleBigMouse.iss --
; The package is the Rust binaries and lbm-app's pictures, nothing else: no managed
; code, so no runtime to install first.
;
; SourceDir is where they are read from. It defaults to the folder CI stages them in
; (dotnet-desktop.yml, "Stage the package"), which a manual compile has to fill the
; same way; the release workflow overrides it (ISCC /DSourceDir=...) to point at the
; binaries SignPath sent back, because the installer has to carry the *signed* ones.
#ifndef SourceDir
  #define SourceDir "..\artifacts\windows"
#endif

; AppVer is normally passed by CI from the git tag (ISCC /DAppVer=5.3.0-beta.2)
; so betas are distinguishable in Programs & Features and in the installer name.
; Falls back to the agent's numeric FileVersion for a manual local compile.
#ifndef AppVer
  #define AppVer GetVersionNumbersString(SourceDir + '\lbm-agent.exe')
#endif

[Setup]
AppId={{C170D4ED-CDCC-4383-8907-B85461E643FF}
AppName=Little Big Mouse
AppVersion={#AppVer}
DefaultDirName={commonpf}\LittleBigMouse
DefaultGroupName=Little Big Mouse
UninstallDisplayIcon={app}\lbm-app.exe
; Give the setup .exe the LBM icon, which this installer did not do at first. The
; same source file lbm-app embeds (crates/lbm-app/build.rs).
SetupIconFile=..\LittleBigMouse.Ui\LittleBigMouse.Ui.Avalonia\Assets\lbm-logo.ico
Compression=lzma2
SolidCompression=yes
OutputDir="."
ArchitecturesInstallIn64BitMode=x64
OutputBaseFilename=LittleBigMouse_{#AppVer}
; Force-close the running app instead of asking the user: the Restart Manager's
; polite WM_CLOSE is treated as "minimize to tray" by the 5.x UI and ignored by the
; windowless hook, so the default (CloseApplications=yes) leaves files locked.
; A hard taskkill in [Code] backs this up (see StopLittleBigMouse).
CloseApplications=force
RestartApplications=no

[Tasks]
; Off unless the user ticks it (docs/multi-pc-service-windows.md): a Windows service
; that lets an approved master drive this PC everywhere, the UAC prompt and the lock
; screen included. Masters are approved one by one, by an administrator, with
; "lbm-service trust"; with none approved the service listens to nobody.
Name: "secureinput"; Description: "Let another PC (multi-PC master) drive this one everywhere, UAC prompts and the lock screen included (Windows service, approved masters only)"; Flags: unchecked

[Files]
; Named one by one: a binary missing from SourceDir stops the compile, and nothing
; that merely happens to lie there is shipped. These five are what SIGNING.md signs.
Source: "{#SourceDir}\lbm-app.exe"; DestDir: "{app}"; Check: Is64BitInstallMode; Flags: ignoreversion
Source: "{#SourceDir}\lbm-agent.exe"; DestDir: "{app}"; Check: Is64BitInstallMode; Flags: ignoreversion
Source: "{#SourceDir}\LittleBigMouse.Hook.exe"; DestDir: "{app}"; Check: Is64BitInstallMode; Flags: ignoreversion
Source: "{#SourceDir}\lbm-service.exe"; DestDir: "{app}"; Check: Is64BitInstallMode; Flags: ignoreversion
Source: "{#SourceDir}\lbm-inject.exe"; DestDir: "{app}"; Check: Is64BitInstallMode; Flags: ignoreversion
; lbm-app's pictures (manufacturer logos, LBM icons), read from icons\ beside it. CI
; stages them there; a wildcard that matched nothing would stop the compile, so an
; installer cannot be built without them.
Source: "{#SourceDir}\icons\*"; DestDir: "{app}\icons"; Check: Is64BitInstallMode; Flags: recursesubdirs ignoreversion

[InstallDelete]
; Old NSIS install (<= 5.2.3) lived in the same folder; drop its leftover uninstaller.
Type: files; Name: "{app}\Uninstall.exe"
; The C# application of 5.x (and of the first v6 builds), replaced by lbm-app: Inno
; does not remove on an update what a new version no longer ships. Left there, its
; window would still start from an old logon task or a pinned shortcut, beside the
; new agent. Its libraries have no list that holds across versions, so they go by
; pattern, which is only safe in a folder that is Little Big Mouse's own and did hold
; that application (HadCSharpApplication). This installer ships no .dll.
Type: files; Name: "{app}\*.dll"; Check: HadCSharpApplication
Type: files; Name: "{app}\*.deps.json"; Check: HadCSharpApplication
Type: files; Name: "{app}\*.runtimeconfig.json"; Check: HadCSharpApplication
Type: filesandordirs; Name: "{app}\runtimes"; Check: HadCSharpApplication
; The satellite folders of Microsoft.Win32.TaskScheduler, then the folders if that
; left them empty.
Type: files; Name: "{app}\de\*.resources.dll"; Check: HadCSharpApplication
Type: dirifempty; Name: "{app}\de"; Check: HadCSharpApplication
Type: files; Name: "{app}\es\*.resources.dll"; Check: HadCSharpApplication
Type: dirifempty; Name: "{app}\es"; Check: HadCSharpApplication
Type: files; Name: "{app}\fr\*.resources.dll"; Check: HadCSharpApplication
Type: dirifempty; Name: "{app}\fr"; Check: HadCSharpApplication
Type: files; Name: "{app}\it\*.resources.dll"; Check: HadCSharpApplication
Type: dirifempty; Name: "{app}\it"; Check: HadCSharpApplication
Type: files; Name: "{app}\ja\*.resources.dll"; Check: HadCSharpApplication
Type: dirifempty; Name: "{app}\ja"; Check: HadCSharpApplication
Type: files; Name: "{app}\pl\*.resources.dll"; Check: HadCSharpApplication
Type: dirifempty; Name: "{app}\pl"; Check: HadCSharpApplication
Type: files; Name: "{app}\ru\*.resources.dll"; Check: HadCSharpApplication
Type: dirifempty; Name: "{app}\ru"; Check: HadCSharpApplication
Type: files; Name: "{app}\sv\*.resources.dll"; Check: HadCSharpApplication
Type: dirifempty; Name: "{app}\sv"; Check: HadCSharpApplication
Type: files; Name: "{app}\tr\*.resources.dll"; Check: HadCSharpApplication
Type: dirifempty; Name: "{app}\tr"; Check: HadCSharpApplication
Type: files; Name: "{app}\zh-CN\*.resources.dll"; Check: HadCSharpApplication
Type: dirifempty; Name: "{app}\zh-CN"; Check: HadCSharpApplication
Type: files; Name: "{app}\zh-Hant\*.resources.dll"; Check: HadCSharpApplication
Type: dirifempty; Name: "{app}\zh-Hant"; Check: HadCSharpApplication
; Its own executables, by name. Last: the first one is what the checks above look for.
Type: files; Name: "{app}\LittleBigMouse.Ui.Loader.exe"
Type: files; Name: "{app}\LittleBigMouse.Ui.Avalonia.exe"

[Registry]
; Remove the stale "LittleBigMouse" (NSIS, <= 5.2.3) Programs & Features entry so it
; no longer shows up next to the Inno-installed one. Cover both registry views.
Root: HKLM64; Subkey: "Software\Microsoft\Windows\CurrentVersion\Uninstall\LittleBigMouse"; Flags: deletekey; Check: IsWin64
Root: HKLM32; Subkey: "Software\Microsoft\Windows\CurrentVersion\Uninstall\LittleBigMouse"; Flags: deletekey

; The window is lbm-app (egui): it starts the agent beside it when none runs, and the
; agent's tray Open and logon task come back to it.
[Icons]
Name: "{group}\Little Big Mouse"; Filename: "{app}\lbm-app.exe"

[Run]
Filename: {app}\lbm-app.exe; Description: Run Application; Flags: postinstall nowait skipifsilent runascurrentuser

[Code]
var
  CSharpApplicationFound: Boolean;

{ Whether the folder installed into is Little Big Mouse's own and held the C#
  application, as it was before this install touched it (set by PrepareToInstall, since
  the InstallDelete section removes the file looked for). The folder's name is the guard for the
  patterns deleted on this answer: Setup appends it to a folder picked in the wizard. }
function HadCSharpApplication: Boolean;
begin
  Result := CSharpApplicationFound;
end;

procedure KillImage(const ExeName: String);
var
  ResultCode: Integer;
begin
  Exec(ExpandConstant('{sys}\taskkill.exe'), '/F /IM ' + ExeName, '',
       SW_HIDE, ewWaitUntilTerminated, ResultCode);
end;

{ The input service of the multi-PC mode (docs/multi-pc-service-windows.md). It runs
  as LocalSystem, so it must sit where only administrators can change it: the service
  checks its folder itself and refuses to start otherwise, and this installer does not
  create it outside Program Files. Its folder under ProgramData (identity and approved
  masters) is made by the service, with a protected DACL, and kept across updates:
  removing it would forget the approved masters and give the service a new identity,
  which the masters that pinned the old one would refuse. }
const
  InputServiceName = 'LittleBigMouseInput';
  InputServiceRuleName = 'Little Big Mouse (input service)';
  InputServicePort = '24818';

function RunSystem(const Exe, Params: String): Boolean;
var
  ResultCode: Integer;
begin
  Result := Exec(ExpandConstant('{sys}\' + Exe), Params, '', SW_HIDE,
                 ewWaitUntilTerminated, ResultCode) and (ResultCode = 0);
  if not Result then
    Log(Exe + ' ' + Params + ' failed, exit code ' + IntToStr(ResultCode));
end;

{ net stop waits for the service to stop, which sc stop does not. Missing or stopped
  service: a logged no-op. }
procedure StopInputService;
begin
  RunSystem('net.exe', 'stop ' + InputServiceName);
end;

function InProgramFiles: Boolean;
begin
  Result := Pos(Lowercase(AddBackslash(ExpandConstant('{commonpf}'))),
                Lowercase(AddBackslash(ExpandConstant('{app}')))) = 1;
end;

procedure RemoveInputService;
begin
  StopInputService;
  RunSystem('sc.exe', 'delete ' + InputServiceName);
  RunSystem('netsh.exe', 'advfirewall firewall delete rule name="' + InputServiceRuleName + '"');
end;

function InputServiceExists: Boolean;
begin
  Result := RunSystem('sc.exe', 'query ' + InputServiceName);
end;

{ Said in a message box and in the log, and in the exit code (GetCustomSetupExitCode
  below): a silent install with /SUPPRESSMSGBOXES answers the box by itself, and a
  deployment tool reads the code. The files stay installed: by then they are in place. }
var
  InputServiceFailure: Boolean;

procedure InputServiceFailed(const Step: String);
begin
  InputServiceFailure := True;
  Log('Input service: ' + Step + ' failed');
  SuppressibleMsgBox('Little Big Mouse input service: ' + Step + ' failed. ' +
                     'Another PC will not be able to drive this one on the secure desktop ' +
                     'until it is fixed; see the setup log, or run the setup again.',
                     mbError, MB_OK, IDOK);
end;

procedure InstallInputService;
var
  Exe, Config: String;
begin
  Exe := ExpandConstant('{app}\lbm-service.exe');
  Config := ' binPath= "\"' + Exe + '\" run" start= delayed-auto' +
            ' DisplayName= "Little Big Mouse input service"';
  { An existing service is reconfigured, not deleted and made again: a deletion only
    marks it while anything holds a handle to it (services.msc, a monitoring tool),
    and the create that followed would fail on the name, then lose the service when
    that handle closed. }
  if InputServiceExists then begin
    StopInputService;
    if not RunSystem('sc.exe', 'config ' + InputServiceName + Config) then begin
      InputServiceFailed('updating the service');
      Exit;
    end;
  end else if not RunSystem('sc.exe', 'create ' + InputServiceName + Config) then begin
    InputServiceFailed('creating the service');
    Exit;
  end;
  RunSystem('sc.exe', 'description ' + InputServiceName +
            ' "Lets an approved multi-PC master drive this PC everywhere, UAC prompts and the lock screen included."');
  { Restarted after a crash twice at most, then left stopped: the SCM repeats the last
    action for every later failure, hence the third one, which does nothing. The count
    starts again after a day without failure. }
  RunSystem('sc.exe', 'failure ' + InputServiceName +
            ' reset= 86400 actions= restart/5000/restart/30000/""/0');
  { Same shape as the agent's rule: inbound TCP, this port, this program, the local
    subnet; the link is mutual TLS and admits approved masters only. Deleted first:
    "add rule" never replaces. }
  RunSystem('netsh.exe', 'advfirewall firewall delete rule name="' + InputServiceRuleName + '"');
  if not RunSystem('netsh.exe', 'advfirewall firewall add rule name="' + InputServiceRuleName + '"' +
                   ' dir=in action=allow protocol=TCP localport=' + InputServicePort +
                   ' program="' + Exe + '" remoteip=localsubnet profile=any enable=yes') then
    InputServiceFailed('the firewall rule');
  { It refuses to start where others than administrators can change its files. }
  if not RunSystem('net.exe', 'start ' + InputServiceName) then
    InputServiceFailed('starting the service');
end;

{ Setup's exit code: 0 as usual, InputServiceExitCode when the input service task was
  ticked and one of its steps failed. Inno's own codes are 1 to 8. }
const
  InputServiceExitCode = 100;

function GetCustomSetupExitCode: Integer;
begin
  if InputServiceFailure then
    Result := InputServiceExitCode
  else
    Result := 0;
end;

procedure StopLittleBigMouse;
begin
  { Hard-kill the running app (UI loader, UI, agent, and the elevated hook) so their
    files unlock before we copy/remove them. Kill by EXACT image name only: a wildcard
    like "LittleBigMouse*" would also match the setup EXE itself
    (LittleBigMouse_<ver>.exe) and the installer would terminate itself mid-install.
    The installer runs elevated, so it can end the elevated hook. Missing processes just
    make taskkill a no-op.
    Supervisors go first: the v6 agent restarts a hook that stops answering, detached,
    so a hook killed while the agent still runs comes straight back and keeps its exe
    locked (seen on a test uninstall: the folder stayed behind with a hook started
    during the uninstall). Same for the UI, which starts an agent when none answers. }
  { The service first, through the SCM (a kill would be taken for a crash and the
    service restarted), and with it the injector it launched. }
  StopInputService;
  KillImage('lbm-inject.exe');
  KillImage('LittleBigMouse.Ui.Loader.exe');
  KillImage('LittleBigMouse.Ui.Avalonia.exe');
  KillImage('lbm-app.exe');
  KillImage('lbm-agent.exe');
  KillImage('LittleBigMouse.Hook.exe');
  KillImage('LittleBigMouse_Daemon.exe');
end;

{ Multi-PC mode (docs/multi-pc.md): the agent listens on TCP 24817 for the other
  machine, and Windows Firewall drops that inbound connection by default. The rule
  is scoped as tightly as the feature allows: inbound TCP only, the default port
  only, the agent executable only, and peers on the local subnet only.
  It is not limited to the private and domain profiles, though that was the first
  idea: Windows 11 files a newly joined network as Public until the user says
  otherwise, and the test machine this was tried on was on exactly such a network,
  so the rule would have stayed silently inactive where the feature is used. The
  local-subnet scope keeps the port shut to anything routed from further away, and
  the link itself is mutual TLS with pinned certificates (lbm-net), so a stranger
  on the same subnet gets a refused handshake, not the input stream.
  netsh rather than New-NetFirewallRule: it answers the same on every supported
  Windows, with no PowerShell execution policy in the way. The rule is named so
  that uninstall, and a reinstall over an older version, find exactly this one.
  A second rule lets in the discovery datagrams (lbm-net/src/discovery.rs): the
  agents of the local network say who they are on UDP 24817, a name, a fingerprint
  and a port, and pairing stays the TLS one above. Same scope, UDP instead of TCP:
  without it, a query broadcast by another machine never reaches this one, which is
  then only seen when it asks itself. }
const
  FirewallRuleName = 'Little Big Mouse (multi-PC)';
  DiscoveryRuleName = 'Little Big Mouse (multi-PC discovery)';
  MultiPcPort = '24817';

function RunNetsh(const Params: String): Boolean;
var
  ResultCode: Integer;
begin
  Result := Exec(ExpandConstant('{sys}\netsh.exe'), Params, '', SW_HIDE,
                 ewWaitUntilTerminated, ResultCode) and (ResultCode = 0);
  if not Result then
    Log('netsh ' + Params + ' failed, exit code ' + IntToStr(ResultCode));
end;

procedure RemoveFirewallRule;
begin
  { netsh exits 1 when no rule matches: a no-op, not an error worth reporting. }
  RunNetsh('advfirewall firewall delete rule name="' + FirewallRuleName + '"');
  RunNetsh('advfirewall firewall delete rule name="' + DiscoveryRuleName + '"');
end;

procedure AddFirewallRule;
begin
  { Delete first: "add rule" never replaces, so a reinstall would stack copies. }
  RemoveFirewallRule;
  RunNetsh('advfirewall firewall add rule name="' + FirewallRuleName + '"' +
           ' dir=in action=allow protocol=TCP localport=' + MultiPcPort +
           ' program="' + ExpandConstant('{app}\lbm-agent.exe') + '"' +
           ' remoteip=localsubnet profile=any enable=yes');
  RunNetsh('advfirewall firewall add rule name="' + DiscoveryRuleName + '"' +
           ' dir=in action=allow protocol=UDP localport=' + MultiPcPort +
           ' program="' + ExpandConstant('{app}\lbm-agent.exe') + '"' +
           ' remoteip=localsubnet profile=any enable=yes');
end;

{ A 5.x logon task (LittleBigMouse_<domain>_<user>) launches the C# window, which this
  install removes: every logon would then fail until an agent had run and pointed the
  task back at itself (crates/lbm-agent/src/schtask.rs), and a silent install runs
  nothing afterwards. So the tasks that launch that very file are pointed at the agent
  here, every account's: only the program changes, the trigger, the account and the
  settings stay. The run level stays too, except outside Program Files, where an
  elevated task would hand administrator rights to whoever can replace the agent. A
  task this cannot rewrite is logged and left to the agent. }
{ What RetargetLogonTask was doing when a COM call failed: the Task Scheduler's
  exceptions often come back with no message, and the log has to say which call. }
var
  RetargetStep: String;

procedure RetargetLogonTask(Folder, Task: Variant);
var
  Definition, Action, Principal: Variant;
  Name, Launched, Agent, Params: String;
begin
  RetargetStep := 'reading a task';
  Name := Task.Name;
  if Pos('LittleBigMouse_', Name) <> 1 then
    Exit;
  RetargetStep := 'reading ' + Name;
  Definition := Task.Definition;
  if Definition.Actions.Count <> 1 then
    Exit;
  Action := Definition.Actions.Item(1);
  Launched := Action.Path;
  if CompareText(RemoveQuotes(Launched),
                 ExpandConstant('{app}\LittleBigMouse.Ui.Avalonia.exe')) <> 0 then
    Exit;
  RetargetStep := 'changing the program of ' + Name;
  Agent := ExpandConstant('{app}\lbm-agent.exe');
  try
    Action.Path := Agent;
    Action.Arguments := '';
    Action.WorkingDirectory := ExpandConstant('{app}');
    Principal := Definition.Principal;
    { TASK_RUNLEVEL_LUA }
    if not InProgramFiles then
      Principal.RunLevel := 0;
    { TASK_CREATE_OR_UPDATE, the account and logon type the task already has, no
      password: Unassigned leaves the arguments out (Null would pass VT_NULL). }
    RetargetStep := 'registering ' + Name + ' through COM';
    Folder.RegisterTaskDefinition(Name, Definition, 6, Unassigned, Unassigned,
                                  Principal.LogonType);
    Log('Logon task ' + Name + ' now launches the agent');
  except
    { Seen on a GitHub runner: the registration throws, with no message. schtasks
      changes the program alone and keeps the account, the trigger and the rest. }
    Log('Logon task ' + Name + ': ' + RetargetStep + ' failed (' + GetExceptionMessage +
        '), trying schtasks');
    RetargetStep := 'changing ' + Name + ' through schtasks';
    Params := '/Change /TN "' + Name + '" /TR "\"' + Agent + '\""';
    if not InProgramFiles then
      Params := Params + ' /RL LIMITED';
    if not RunSystem('schtasks.exe', Params) then
      RaiseException('schtasks /Change failed');
    Log('Logon task ' + Name + ' now launches the agent (schtasks)');
  end;
end;

procedure RetargetLogonTasks;
var
  Service, Folder, Tasks: Variant;
  I: Integer;
begin
  try
    Service := CreateOleObject('Schedule.Service');
    Service.Connect;
    Folder := Service.GetFolder('\');
    { TASK_ENUM_HIDDEN }
    Tasks := Folder.GetTasks(1);
    for I := 1 to Tasks.Count do
      try
        RetargetLogonTask(Folder, Tasks.Item(I));
      except
        Log('A logon task could not be pointed at the agent (' + RetargetStep + '): ' +
            GetExceptionMessage);
      end;
  except
    Log('The logon tasks could not be read: ' + GetExceptionMessage);
  end;
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssPostInstall then begin
    RetargetLogonTasks;
    AddFirewallRule;
    if not WizardIsTaskSelected('secureinput') then
      { Unticked on an update: a service installed before goes, its folder stays. }
      RemoveInputService
    else if InProgramFiles then
      InstallInputService
    else begin
      { Asked for and not installed: a failure for the exit code too. }
      InputServiceFailure := True;
      Log('Input service: not installed, the folder is not under Program Files');
      SuppressibleMsgBox('The input service runs with system rights, so it is installed only ' +
                         'in a folder under ' + ExpandConstant('{commonpf}') + '. ' +
                         'It was not installed.', mbInformation, MB_OK, IDOK);
    end;
  end;
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  StopLittleBigMouse;
  CSharpApplicationFound :=
    (CompareText(ExtractFileName(RemoveBackslashUnlessRoot(ExpandConstant('{app}'))), 'LittleBigMouse') = 0)
    and FileExists(ExpandConstant('{app}\LittleBigMouse.Ui.Avalonia.exe'));
  Result := '';
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usUninstall then begin
    RemoveInputService;
    StopLittleBigMouse;
    RemoveFirewallRule;
  end;
  { Only a real uninstall gets here, never an update: the approved masters and the
    service's identity go, and the offer it published for the agents. }
  if CurUninstallStep = usPostUninstall then begin
    DelTree(ExpandConstant('{commonappdata}\LittleBigMouse\service'), True, True, True);
    { Only if empty: nothing else of ours is kept there yet, but something may be one day. }
    RemoveDir(ExpandConstant('{commonappdata}\LittleBigMouse'));
    RegDeleteKeyIncludingSubkeys(HKLM64, 'SOFTWARE\LittleBigMouse\InputService');
    { The service writes its offer there, and nothing else of ours lives under it yet. }
    RegDeleteKeyIfEmpty(HKLM64, 'SOFTWARE\LittleBigMouse');
  end;
end;

