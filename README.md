# Rearguard

Rearguard is a Windows background enforcer for blocking a compiled list of game
processes. While it is running, it scans every two seconds and terminates every
matching process it finds.

It also disables and stops the `vgc` and `vgk` Windows services on every scan.
The compiled process list currently includes Valorant/Vanguard, League Client,
and Genshin Impact.

## Commands

Run the enforcer immediately (the default when no argument is supplied):

```powershell
rearguard.exe run
```

Install a per-user, elevated Task Scheduler entry named `Rearguard` so it starts
at sign-in:

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
