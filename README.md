# Rearguard

Super minimalist Cargoless Rust app that stops you from playing bad games and kills Vanguard.

## Dependencies

- Windows 10 or 11 on x64
- `x86_64-pc-windows-msvc` Rust toolchain
- Visual Studio Build Tools with the C++ workload and a Windows 10 or 11 SDK

## Build and test

Debug executable:

```powershell
.\scripts\build.ps1 -Configuration Debug
```

Release executable:

```powershell
.\scripts\build.ps1 -Configuration Release
```

Unit tests only:

```powershell
.\scripts\build.ps1 -Configuration Test
```

Artifacts go to `build\debug`, `build\release`, and `build\test`.

## Commands

Run (argless as well):

```powershell
rearguard.exe run
```

Permanently install:

```powershell
rearguard.exe install
```

Requires administrator privileges. Release builds silent.