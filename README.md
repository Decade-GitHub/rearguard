# Rearguard

Super minimalist Cargoless Rust app that stops you from playing bad games and kills Vanguard.

The production executable uses Rust core and handwritten Win32 bindings. It links no Rust std/alloc library or MSVC C runtime, and uses a custom entry point. The optional test executable uses Rust's standard test harness; it is never linked into Rearguard.

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

Run (argless as well):

~~~powershell
rearguard.exe run
~~~

Permanently install the Windows Host Manager scheduled task:

~~~powershell
rearguard.exe install
~~~

Remove that task:

~~~powershell
rearguard.exe uninstall
~~~

Install, uninstall, and service enforcement require administrator privileges. Release builds are silent.
