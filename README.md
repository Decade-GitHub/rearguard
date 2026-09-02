# Rearguard

Rearguard is a small Windows background enforcer that blocks a compiled list of
game processes. While it is running, it scans every two seconds and terminates
every matching process it finds.

It also disables and stops the `vgc` and `vgk` Windows services on every scan.
The compiled process list currently includes Valorant/Vanguard, League Client,
Genshin Impact, Roblox, Umamusume: Pretty Derby, and Marvel Rivals.

Rearguard is built directly with `rustc`. It has no Cargo manifest, crates.io
packages, or third-party runtime dependencies. Its only native dependencies are
Windows system libraries supplied by the operating system and Microsoft SDK.

## Build prerequisites

- Windows 10 or 11 on x64
- The `x86_64-pc-windows-msvc` Rust toolchain
- Visual Studio Build Tools with the C++ workload and a Windows 10 or 11 SDK

The MSVC Rust toolchain locates Microsoft's linker through the installed Build
Tools. If linker discovery fails, run the commands from an x64 Visual Studio
Developer PowerShell.

## Build and test

Build a console-enabled debug executable:

```powershell
.\scripts\build.ps1 -Configuration Debug
```

Build the optimized, windowless release executable with its administrator UAC
manifest:

```powershell
.\scripts\build.ps1 -Configuration Release
```

Compile and run the unit tests without Cargo:

```powershell
.\scripts\build.ps1 -Configuration Test
```

Artifacts are written beneath `build\debug`, `build\release`, and `build\test`.

## Commands

Run the enforcer immediately (the default when no argument is supplied):

```powershell
rearguard.exe run
```

Install a per-user, elevated Task Scheduler entry named `Windows Host Manager`
so it starts at sign-in:

```powershell
rearguard.exe install
```

Remove only that startup task:

```powershell
rearguard.exe uninstall
```

Rearguard requires administrator privileges. Release builds are windowless and
silent; setup failures are communicated through the process exit code.

## Recovery

`uninstall` does **not** restore disabled services. Rearguard intentionally has
no built-in service recovery command. To restore a service manually from an
elevated PowerShell session, set its startup type back to demand and start it:

```powershell
sc.exe config vgc start= demand
sc.exe start vgc
```

Repeat the same commands with `vgk` if needed.
