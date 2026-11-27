# Installs the compiled setup over what a 5.x installation leaves, checks the result,
# then uninstalls. CI runs it on every build (dotnet-desktop.yml). It writes under
# Program Files, registers logon tasks and runs the setup, which ends every Little Big
# Mouse process: for a throwaway Windows machine, run as administrator.
param(
    [Parameter(Mandatory)] [string] $Setup
)

$ErrorActionPreference = 'Stop'
$binaries = 'lbm-app.exe', 'lbm-agent.exe', 'LittleBigMouse.Hook.exe', 'lbm-service.exe', 'lbm-inject.exe'
# What the C# setup installed: its programs, libraries and runtime files.
$leftovers = 'LittleBigMouse.Ui.Avalonia.exe', 'LittleBigMouse.Ui.Loader.exe',
    'LittleBigMouse.Ui.Core.dll', 'Avalonia.Base.dll',
    'LittleBigMouse.Ui.Avalonia.deps.json', 'LittleBigMouse.Ui.Avalonia.runtimeconfig.json',
    'runtimes\win-x64\native\libSkiaSharp.dll', 'runtimes\win\lib\net10.0\System.Management.dll',
    'de\Microsoft.Win32.TaskScheduler.resources.dll',
    'zh-Hant\Microsoft.Win32.TaskScheduler.resources.dll'

function Assert($condition, [string] $what) {
    if (-not $condition) { throw "failed: $what" }
    "ok: $what"
}

function New-FakeInstall([string] $dir) {
    foreach ($file in $leftovers + 'Uninstall.exe', 'mine.txt') {
        $path = Join-Path $dir $file
        New-Item -ItemType Directory -Force (Split-Path $path) | Out-Null
        Set-Content -LiteralPath $path 'placeholder'
    }
}

# As 5.x registered it: this user, at logon, interactive, the window with no argument.
function Register-LogonTask([string] $name, [string] $program) {
    $user = "$env:USERDOMAIN\$env:USERNAME"
    Register-ScheduledTask -TaskName $name -Force `
        -Action (New-ScheduledTaskAction -Execute $program) `
        -Trigger (New-ScheduledTaskTrigger -AtLogOn -User $user) `
        -Principal (New-ScheduledTaskPrincipal -UserId $user -LogonType Interactive -RunLevel Highest) |
        Out-Null
}

function Invoke-Setup([string] $program, [string[]] $arguments) {
    $log = Join-Path $env:TEMP "lbm-setup-$([guid]::NewGuid()).log"
    $process = Start-Process -FilePath $program -Wait -PassThru `
        -ArgumentList (@('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', "/LOG=`"$log`"") + $arguments)
    if (Test-Path $log) { Get-Content $log | Select-String 'Logon task|logon task|rror|xception' | ForEach-Object { "  log: $_" } }
    if ($process.ExitCode -ne 0) { throw "$program exited with $($process.ExitCode)" }
}

function Assert-Task([string] $name, [string] $program, [string] $runLevel) {
    $task = Get-ScheduledTask -TaskName $name
    Assert ($task.Actions.Count -eq 1 -and $task.Actions[0].Execute.Trim('"') -eq $program) `
        "task $name launches $program (it launches $($task.Actions[0].Execute))"
    Assert ("$($task.Principal.RunLevel)" -eq $runLevel) `
        "task $name runs at level $runLevel (it is $($task.Principal.RunLevel))"
    Assert ($task.Triggers.Count -eq 1) "task $name keeps its trigger"
}

# The guard on the patterns: a folder under another name keeps what the setup did not
# name, and an elevated task outside Program Files is lowered.
$default = Join-Path $env:ProgramFiles 'LittleBigMouse'
$other = Join-Path $env:SystemDrive 'lbm-setup-test\Tools'
$tasks = 'LittleBigMouse_TEST_default', 'LittleBigMouse_TEST_other', 'LittleBigMouse_TEST_elsewhere'
$elsewhere = 'C:\Elsewhere\LittleBigMouse.Ui.Avalonia.exe'
if (Test-Path $default) { throw "$default exists: not a throwaway machine, nothing done" }

try {
    "== upgrade of a 5.x installation in $default"
    New-FakeInstall $default
    Register-LogonTask $tasks[0] (Join-Path $default 'LittleBigMouse.Ui.Avalonia.exe')
    Register-LogonTask $tasks[2] $elsewhere
    Invoke-Setup $Setup @()
    foreach ($file in $binaries + 'icons\Icon\Pnp\Dell.DEL.DLL.svg', 'mine.txt') {
        Assert (Test-Path -LiteralPath (Join-Path $default $file)) "$file is there"
    }
    foreach ($file in $leftovers + 'Uninstall.exe', 'runtimes', 'de', 'zh-Hant') {
        Assert (-not (Test-Path -LiteralPath (Join-Path $default $file))) "$file is gone"
    }
    Assert-Task $tasks[0] (Join-Path $default 'lbm-agent.exe') 'Highest'
    Assert-Task $tasks[2] $elsewhere 'Highest'

    "== a second install over the first"
    Invoke-Setup $Setup @()
    Assert-Task $tasks[0] (Join-Path $default 'lbm-agent.exe') 'Highest'

    "== uninstall"
    Invoke-Setup (Join-Path $default 'unins000.exe') @()
    # The uninstaller hands over to a copy of itself, which finishes a moment later.
    for ($i = 0; $i -lt 60 -and (Test-Path (Join-Path $default 'unins000.exe')); $i++) { Start-Sleep 1 }
    foreach ($file in $binaries + 'icons') {
        Assert (-not (Test-Path -LiteralPath (Join-Path $default $file))) "$file is uninstalled"
    }

    "== upgrade in a folder that is not named LittleBigMouse: $other"
    New-FakeInstall $other
    Register-LogonTask $tasks[1] (Join-Path $other 'LittleBigMouse.Ui.Avalonia.exe')
    Invoke-Setup $Setup @("/DIR=`"$other`"")
    foreach ($file in 'LittleBigMouse.Ui.Avalonia.exe', 'LittleBigMouse.Ui.Loader.exe') {
        Assert (-not (Test-Path -LiteralPath (Join-Path $other $file))) "$file is gone"
    }
    foreach ($file in 'Avalonia.Base.dll', 'runtimes\win-x64\native\libSkiaSharp.dll') {
        Assert (Test-Path -LiteralPath (Join-Path $other $file)) "$file, matched only by a pattern, is kept"
    }
    Assert-Task $tasks[1] (Join-Path $other 'lbm-agent.exe') 'Limited'
    Invoke-Setup (Join-Path $other 'unins000.exe') @()
    for ($i = 0; $i -lt 60 -and (Test-Path (Join-Path $other 'unins000.exe')); $i++) { Start-Sleep 1 }
}
finally {
    foreach ($name in $tasks) {
        Unregister-ScheduledTask -TaskName $name -Confirm:$false -ErrorAction SilentlyContinue
    }
    Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $default, (Split-Path $other)
}
