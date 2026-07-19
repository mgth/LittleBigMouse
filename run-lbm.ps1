<#
  run-lbm.ps1 — Dev launcher for the in-progress LittleBigMouse build.

  Stops any running instance, builds the Rust hook daemon + the Avalonia UI,
  stages the daemon next to the UI (lbm-hook.exe -> LittleBigMouse.Hook.exe),
  and relaunches the app elevated (the app manifest is requireAdministrator).

  The script self-elevates with a SINGLE UAC prompt: the real work runs in the
  elevated console window, which stays open at the end so you can read the build
  output (or any error).

  Usage (from anywhere):
    powershell -ExecutionPolicy Bypass -File C:\dev\LittleBigMouse\run-lbm.ps1

  Options:
    -Config <Release|Debug|ReleaseDebug>  build configuration (default: Release)
    -NoBuild      skip compilation, just (re)stage the daemon and relaunch
    -SkipDaemon   don't build/stage the Rust daemon (UI only)
    -NoLaunch     build/stage only, don't launch the app
#>
[CmdletBinding()]
param(
    [ValidateSet('Release','Debug','ReleaseDebug')][string]$Config = 'Release',
    [switch]$NoBuild,
    [switch]$SkipDaemon,
    [switch]$NoLaunch
)

$ErrorActionPreference = 'Stop'

# --- self-elevate (single UAC prompt) -----------------------------------------
$principal = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltinRole]::Administrator)) {
    $argList = @('-NoProfile','-ExecutionPolicy','Bypass','-File',"`"$PSCommandPath`"",'-Config',$Config)
    if ($NoBuild)    { $argList += '-NoBuild' }
    if ($SkipDaemon) { $argList += '-SkipDaemon' }
    if ($NoLaunch)   { $argList += '-NoLaunch' }
    Start-Process powershell -Verb RunAs -ArgumentList $argList
    return
}

# --- paths --------------------------------------------------------------------
$root       = $PSScriptRoot
$uiProj     = Join-Path $root 'LittleBigMouse.Ui\LittleBigMouse.Ui.Avalonia\LittleBigMouse.Ui.Avalonia.csproj'
$binDir     = Join-Path $root ("LittleBigMouse.Ui\LittleBigMouse.Ui.Avalonia\bin\{0}\net10.0" -f $Config)
$uiExe      = Join-Path $binDir 'LittleBigMouse.Ui.Avalonia.exe'
$rustDir    = Join-Path $root 'LittleBigMouse-Hook-Rust'
$rustExe    = Join-Path $rustDir 'target\debug\lbm-hook.exe'
$stagedHook = Join-Path $binDir 'LittleBigMouse.Hook.exe'

# Dev version stamp: v5.4.1 + 12 commits -> 5.4.1.12, so the About window tells
# dev builds apart from releases. Empty (Directory.Build.props version applies)
# when git or the tags are unavailable.
$version = ''
try {
    $described = & git -C $root describe --tags --long --match 'v*' 2>$null
    if ($LASTEXITCODE -eq 0 -and $described -match '^v(.+)-(\d+)-g[0-9a-f]+$') {
        $version = '{0}.{1}' -f $Matches[1], $Matches[2]
    }
} catch { }

function Step($m) { Write-Host "`n==> $m" -ForegroundColor Cyan }
function Note($m) { Write-Host "    $m" -ForegroundColor DarkGray }

try {
    # 1. stop running instance(s) (needs elevation — we are elevated here)
    $procs = Get-Process -Name 'LittleBigMouse.Ui.Avalonia','LittleBigMouse.Hook' -ErrorAction SilentlyContinue
    if ($procs) {
        Step ("Stopping running instance(s): {0}" -f ($procs.Id -join ', '))
        $procs | Stop-Process -Force
        Start-Sleep -Seconds 1
    }

    # 2. build the Rust daemon (non-fatal: if cargo is missing or fails, keep going and
    #    stage whatever binary already exists under target\debug).
    if (-not $NoBuild -and -not $SkipDaemon) {
        Step 'Building Rust hook daemon  (cargo build)'
        Push-Location $rustDir
        try {
            & cargo build
            if ($LASTEXITCODE -ne 0) { Write-Host "!!  cargo build failed (exit $LASTEXITCODE) - staging existing binary" -ForegroundColor Yellow }
        }
        catch { Write-Host "!!  cargo not available ($($_.Exception.Message)) - staging existing binary" -ForegroundColor Yellow }
        finally { Pop-Location }
    }

    # 3. build the UI
    #    NOTE: build AnyCPU (no -p:Platform=x64). The project declares <Platforms>x64</Platforms>
    #    for the VS UI, but AnyCPU is what outputs to bin\<Config>\net10.0 — the folder the app
    #    is launched and staged from. Passing -p:Platform=x64 diverts output to bin\x64\... which
    #    is never launched, so the freshly built code would silently never run.
    if (-not $NoBuild) {
        Step ("Building UI  ({0} AnyCPU{1})" -f $Config, $(if ($version) { ", v$version" } else { '' }))
        $buildArgs = @($uiProj, '-c', $Config, '--nologo', '-v', 'm')
        if ($version) { $buildArgs += "-p:Version=$version" }
        & dotnet build @buildArgs
        if ($LASTEXITCODE -ne 0) { throw 'dotnet build failed' }
    }

    # 4. stage the daemon next to the UI
    if (-not $SkipDaemon) {
        if (Test-Path $rustExe) {
            Step 'Staging daemon  ->  LittleBigMouse.Hook.exe'
            Copy-Item $rustExe $stagedHook -Force
            Note $stagedHook
        }
        else { Write-Host "!!  Rust daemon not found: $rustExe (skipping staging)" -ForegroundColor Yellow }
    }

    # 5. launch (already elevated -> the child inherits, no second UAC prompt)
    if (-not $NoLaunch) {
        if (-not (Test-Path $uiExe)) { throw "UI exe not found: $uiExe" }
        Step 'Launching LittleBigMouse (elevated)'
        Start-Process $uiExe
    }

    Write-Host "`nDone." -ForegroundColor Green
}
catch {
    Write-Host "`nFAILED: $($_.Exception.Message)" -ForegroundColor Red
}
finally {
    Write-Host "`nPress Enter to close this window..."
    [void](Read-Host)
}
