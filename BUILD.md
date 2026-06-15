# Building LittleBigMouse

LittleBigMouse is a mixed .NET and native Windows project. The main UI is an
Avalonia .NET application, and the mouse hook is a native C++ executable.

## Prerequisites

- Windows 10 or Windows 11, x64
- Git
- Visual Studio 2022, or Visual Studio Build Tools 2022
- .NET 8 SDK, or a newer .NET SDK that can build `net8.0` projects
- Visual Studio workloads/components:
  - `.NET desktop development`
  - `Desktop development with C++`
  - MSVC v143 C++ build tools
  - Windows 10 or Windows 11 SDK

## Clone

Clone the repository with submodules:

```powershell
git clone --recurse-submodules https://github.com/thomcuddihy/LittleBigMouse.git
cd LittleBigMouse
```

If the repository was cloned without submodules, initialize them before
building:

```powershell
git submodule update --init --recursive
```

## Build With Visual Studio

1. Open `LittleBigMouse.sln` in Visual Studio 2022.
2. Select the `Release` configuration and `x64` platform.
3. Restore NuGet packages when Visual Studio prompts, or run `Restore NuGet Packages`.
4. Build `LittleBigMouse.Hook`.
5. Build `LittleBigMouse.Ui.Avalonia`.

The primary development output is:

```text
LittleBigMouse.Ui\LittleBigMouse.Ui.Avalonia\bin\x64\Release\net8.0\LittleBigMouse.Ui.Avalonia.exe
```

The native hook output is:

```text
LittleBigMouse.Hook\bin\x64\Release\LittleBigMouse.Hook.exe
```

When running from the repository build folders, the UI locates the hook from the
native project output path. If the UI starts but the hook does not, make sure
`LittleBigMouse.Hook.exe` was built for the same configuration and platform.

## Build From The Command Line

Use a **Developer PowerShell for VS 2022** or **x64 Native Tools Command Prompt
for VS 2022** so that MSBuild can find the Visual C++ toolchain and Windows SDK.

Restore the .NET dependencies:

```powershell
dotnet restore LittleBigMouse.Ui\LittleBigMouse.Ui.Avalonia\LittleBigMouse.Ui.Avalonia.csproj
```

Build the native hook:

```powershell
msbuild LittleBigMouse.Hook\LittleBigMouse.Hook.vcxproj /m /p:Configuration=Release /p:Platform=x64
```

Build the Avalonia UI:

```powershell
dotnet build LittleBigMouse.Ui\LittleBigMouse.Ui.Avalonia\LittleBigMouse.Ui.Avalonia.csproj -c Release -p:Platform=x64 --no-restore
```

Run the app from:

```powershell
.\LittleBigMouse.Ui\LittleBigMouse.Ui.Avalonia\bin\x64\Release\net8.0\LittleBigMouse.Ui.Avalonia.exe
```

## Full Solution Builds

Visual Studio can build the full solution when all optional project requirements
are installed. For command-line builds, the two-step build above is the
recommended path for producing the runnable application because it avoids mixing
the native C++ project and SDK-style .NET projects through a single MSBuild
invocation.

If your command-line environment can resolve both Visual C++ and .NET SDK
targets, a full solution build can be attempted with:

```powershell
msbuild LittleBigMouse.sln /restore /m /p:Configuration=Release /p:Platform=x64
```

## Troubleshooting

- `Microsoft.Cpp.Default.props` is missing: install the Visual Studio C++
  workload and run the build from a Visual Studio developer shell.
- `The SDK 'Microsoft.NET.Sdk' specified could not be found`: install the .NET 8
  SDK and confirm `dotnet --info` works in the current shell.
- Output DLLs cannot be copied because they are in use: close
  `LittleBigMouse.Ui.Avalonia.exe` and `LittleBigMouse.Hook.exe`, then rebuild.
- The UI opens but the hook does not start: build `LittleBigMouse.Hook` for
  `Release|x64` and confirm the hook executable exists in
  `LittleBigMouse.Hook\bin\x64\Release`.
- NuGet vulnerability warnings may appear during restore/build. These warnings
  do not necessarily block compilation, but should be reviewed before shipping a
  release.
