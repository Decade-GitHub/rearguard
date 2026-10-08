# Rearguard

Ultra minimalist Rust service app that stops you from playing bad games.


## Dependencies

- Windows 10 or 11 on x64
- x86_64-pc-windows-msvc Rust toolchain
- Visual Studio Build Tools with the C++ workload and a Windows 10 or 11 SDK

## Build and test

Debug executable:

~~~powershell
.\scripts\build.ps1 -Configuration Debug
~~~

Release executable:

~~~powershell
.\scripts\build.ps1 -Configuration Release
~~~

The release script builds both size optimization levels, s and z, and keeps the smaller EXE. It retains the embedded administrator manifest.

Optional unit tests:

~~~powershell
.\scripts\build.ps1 -Configuration Test
~~~

Artifacts go to build\debug, build\release, and build\test.

## Commands

Run (argless default):

~~~powershell
rearguard.exe run
~~~

Permanently install the WHM task:

~~~powershell
rearguard.exe install
~~~

Remove task:

~~~powershell
rearguard.exe uninstall
~~~

Install, uninstall, and service enforcement require administrator privileges. Release builds are silent.
