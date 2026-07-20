# Build the Rust hook daemon in release and stage it under the name the C# UI
# expects. Cargo cannot emit a target named "LittleBigMouse.Hook" (the '.' is
# rejected), so the binary builds as lbm-hook.exe and is copied to
# LittleBigMouse.Hook.exe here.
#
# Usage: .\stage.ps1 [-UiDir <published UI directory>]
param([string]$UiDir)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $MyInvocation.MyCommand.Path
$repo = Split-Path -Parent $root

cargo build --release --manifest-path (Join-Path $root 'Cargo.toml')
$src = Join-Path $root 'target\release\lbm-hook.exe'

if (-not $UiDir) {
  $UiDir = Join-Path $repo 'LittleBigMouse.Ui\LittleBigMouse.Ui.Avalonia\bin\x64\Release\net10.0'
}
New-Item -ItemType Directory -Force -Path $UiDir | Out-Null
Copy-Item $src (Join-Path $UiDir 'LittleBigMouse.Hook.exe') -Force
Write-Host "Staged -> $UiDir\LittleBigMouse.Hook.exe"
