[CmdletBinding()]
param(
    [ValidateSet("Debug", "Release", "Test")]
    [string] $Configuration = "Debug"
)

$ErrorActionPreference = "Stop"

$projectRoot = Split-Path -Parent $PSScriptRoot
$sourcePath = Join-Path $projectRoot "src\main.rs"
$rustcCommand = Get-Command rustc -ErrorAction SilentlyContinue

if ($null -eq $rustcCommand) {
    throw "rustc was not found. Install the x86_64-pc-windows-msvc Rust toolchain and reopen the shell."
}

$rustcDetails = & $rustcCommand.Source -vV
if ($LASTEXITCODE -ne 0) {
    throw "rustc could not report its target information."
}

$hostLine = $rustcDetails | Where-Object { $_ -like "host:*" }
if ($hostLine -ne "host: x86_64-pc-windows-msvc") {
    throw "Rearguard requires the x86_64-pc-windows-msvc Rust toolchain; found '$hostLine'."
}

$commonArguments = @(
    "--edition=2024"
    "--crate-name", "rearguard"
    "--target", "x86_64-pc-windows-msvc"
)

switch ($Configuration) {
    "Debug" {
        $outputDirectory = Join-Path $projectRoot "build\debug"
        $outputPath = Join-Path $outputDirectory "rearguard.exe"
        $rustcArguments = $commonArguments + @(
            "-C", "debug-assertions=yes"
            "-C", "debuginfo=2"
            "-C", "opt-level=0"
            "-o", $outputPath
            $sourcePath
        )
    }
    "Release" {
        $outputDirectory = Join-Path $projectRoot "build\release"
        $outputPath = Join-Path $outputDirectory "rearguard.exe"
        $rustcArguments = $commonArguments + @(
            "-C", "debug-assertions=no"
            "-C", "opt-level=3"
            "-C", 'link-arg=/MANIFESTUAC:level=''requireAdministrator'' uiAccess=''false'''
            "-C", "link-arg=/MANIFEST:EMBED"
            "-o", $outputPath
            $sourcePath
        )
    }
    "Test" {
        $outputDirectory = Join-Path $projectRoot "build\test"
        $outputPath = Join-Path $outputDirectory "rearguard-tests.exe"
        $rustcArguments = $commonArguments + @(
            "--test"
            "-C", "debug-assertions=yes"
            "-C", "debuginfo=2"
            "-C", "opt-level=0"
            "-o", $outputPath
            $sourcePath
        )
    }
}

New-Item -ItemType Directory -Force -Path $outputDirectory | Out-Null

& $rustcCommand.Source @rustcArguments
if ($LASTEXITCODE -ne 0) {
    throw "rustc failed. Ensure the Visual Studio Build Tools C++ workload and a Windows 10 or 11 SDK are installed so the MSVC linker can be discovered."
}

if ($Configuration -eq "Test") {
    & $outputPath
    if ($LASTEXITCODE -ne 0) {
        throw "The Rearguard test executable failed."
    }
}

Write-Host "$Configuration succeeded: $outputPath"
