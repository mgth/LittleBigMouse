# Build the Rust hook daemon in release and stage it under the name the C# UI
# expects. Cargo cannot emit a target named "LittleBigMouse.Hook" (the '.' is
# rejected), so the binary builds as lbm-hook.exe and is copied to
# LittleBigMouse.Hook.exe here.
#
# Usage:
#   .\stage.ps1                       # build + stage under LittleBigMouse.Hook\bin\rust
#   .\stage.ps1 -UiDir <path>         # also stage next to a UI output dir (preferred
#                                     # by FindHookPath, which checks the sibling first)
param([string]$UiDir)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $MyInvocation.MyCommand.Path
$repo = Split-Path -Parent $root

cargo build --release --manifest-path (Join-Path $root 'Cargo.toml')
$src = Join-Path $root 'target\release\lbm-hook.exe'

$binDir = Join-Path $repo 'LittleBigMouse.Hook\bin\rust'
New-Item -ItemType Directory -Force -Path $binDir | Out-Null
Copy-Item $src (Join-Path $binDir 'LittleBigMouse.Hook.exe') -Force
Write-Host "Staged -> $binDir\LittleBigMouse.Hook.exe"

if ($UiDir) {
  Copy-Item $src (Join-Path $UiDir 'LittleBigMouse.Hook.exe') -Force
  Write-Host "Staged -> $UiDir\LittleBigMouse.Hook.exe"
}
