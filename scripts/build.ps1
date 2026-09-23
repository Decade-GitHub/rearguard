[CmdletBinding()]
param(
    [ValidateSet("Debug", "Release", "Test")]
    [string] $Configuration = "Debug"
)

$ErrorActionPreference = "Stop"

$projectRoot = Split-Path -Parent $PSScriptRoot
$sourcePath = Join-Path $projectRoot "src\main.rs"
$testPath = Join-Path $projectRoot "tests\rearguard.rs"
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
    "--target", "x86_64-pc-windows-msvc"
)

$nativeArguments = @(
    "-C", "panic=abort"
    "-C", "lto=fat"
    "-C", "codegen-units=1"
    "-C", "link-arg=/ENTRY:rearguard_entry"
    "-C", "link-arg=/NODEFAULTLIB"
    "-C", "link-arg=/OPT:REF,ICF"
    "-C", "link-arg=/INCREMENTAL:NO"
)

switch ($Configuration) {
    "Debug" {
        $outputDirectory = Join-Path $projectRoot "build\debug"
        $outputPath = Join-Path $outputDirectory "rearguard.exe"
        $rustcArguments = $commonArguments + @("--crate-name", "rearguard") +
            $nativeArguments + @(
                "-C", "debug-assertions=yes"
                "-C", "debuginfo=2"
                "-C", "opt-level=s"
                "-C", "link-arg=/SUBSYSTEM:CONSOLE"
                "-o", $outputPath
                $sourcePath
            )
        New-Item -ItemType Directory -Force -Path $outputDirectory | Out-Null
        & $rustcCommand.Source @rustcArguments
        if ($LASTEXITCODE -ne 0) {
            throw "The no-std Debug build failed."
        }
    }
    "Release" {
        $outputDirectory = Join-Path $projectRoot "build\release"
        $outputPath = Join-Path $outputDirectory "rearguard.exe"
        New-Item -ItemType Directory -Force -Path $outputDirectory | Out-Null

        $candidates = @()
        foreach ($level in @("s", "z")) {
            $candidatePath = Join-Path $outputDirectory "rearguard-$level.exe"
            $rustcArguments = $commonArguments + @("--crate-name", "rearguard") +
                $nativeArguments + @(
                    "-C", "debug-assertions=no"
                    "-C", "debuginfo=0"
                    "-C", "strip=symbols"
                    "-C", "link-arg=/DEBUG:NONE"
                    "-C", "opt-level=$level"
                    "-C", "link-arg=/SUBSYSTEM:WINDOWS"
                    "-C", "link-arg=/MANIFESTUAC:level='requireAdministrator' uiAccess='false'"
                    "-C", "link-arg=/MANIFEST:EMBED"
                    "-o", $candidatePath
                    $sourcePath
                )
            & $rustcCommand.Source @rustcArguments
            if ($LASTEXITCODE -ne 0) {
                throw "The no-std Release build with opt-level=$level failed."
            }
            $candidates += [pscustomobject]@{
                Level = $level
                Path = $candidatePath
                Bytes = (Get-Item -LiteralPath $candidatePath).Length
            }
        }

        $selected = $candidates | Sort-Object Bytes, Level | Select-Object -First 1
        Copy-Item -LiteralPath $selected.Path -Destination $outputPath -Force
        Write-Host "Release candidates: s=$($candidates[0].Bytes) bytes, z=$($candidates[1].Bytes) bytes."
        Write-Host "Selected opt-level=$($selected.Level): $($selected.Bytes) bytes."
        foreach ($candidate in $candidates) {
            Remove-Item -LiteralPath $candidate.Path
            $candidatePdb = [System.IO.Path]::ChangeExtension($candidate.Path, ".pdb")
            if (Test-Path -LiteralPath $candidatePdb) {
                Remove-Item -LiteralPath $candidatePdb
            }
        }
        $oldPdb = Join-Path $outputDirectory "rearguard.pdb"
        if (Test-Path -LiteralPath $oldPdb) {
            Remove-Item -LiteralPath $oldPdb
        }
    }
    "Test" {
        $outputDirectory = Join-Path $projectRoot "build\test"
        $outputPath = Join-Path $outputDirectory "rearguard-tests.exe"
        New-Item -ItemType Directory -Force -Path $outputDirectory | Out-Null
        $rustcArguments = $commonArguments + @(
            "--crate-name", "rearguard_tests"
            "--test"
            "-C", "debug-assertions=yes"
            "-C", "debuginfo=2"
            "-o", $outputPath
            $testPath
        )
        & $rustcCommand.Source @rustcArguments
        if ($LASTEXITCODE -ne 0) {
            throw "The separate Rearguard test build failed."
        }
        & $outputPath
        if ($LASTEXITCODE -ne 0) {
            throw "The Rearguard tests failed."
        }
    }
}

Write-Host "$Configuration succeeded: $outputPath"
