#Requires -Version 7.0
[CmdletBinding()]
param([ValidateSet('Debug', 'Release')][string]$Configuration = 'Release')

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Invoke-Checked {
    param([string]$Executable, [string[]]$Arguments)
    & $Executable @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "$Executable failed with exit code $LASTEXITCODE"
    }
}

$repository = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$compatibility = Join-Path $repository 'apps/windows/compatibility'
$buildRoot = Join-Path $repository 'target/windows-compat'
$nativeTarget = 'x86_64-pc-windows-msvc'
$generator = Join-Path $buildRoot "$nativeTarget/debug/memedock-windows-bindgen.exe"
$nativeLibrary = Join-Path $buildRoot "$nativeTarget/debug/memedock_windows_interop_probe.dll"
$generated = Join-Path $buildRoot 'generated'

if (![OperatingSystem]::IsWindows()) {
    throw 'This integration entry point requires Windows and PowerShell 7.'
}

Push-Location $compatibility
try {
    & (Join-Path $PSScriptRoot 'prepare-bindgen.ps1')
    Invoke-Checked 'cargo' @('fmt', '-p', 'memedock-windows-bindgen', '-p', 'memedock-windows-interop-probe', '--', '--check')
    Invoke-Checked 'cargo' @('build', '--workspace', '--locked', '--target', $nativeTarget, '--target-dir', $buildRoot)
    [IO.Directory]::CreateDirectory($generated) | Out-Null
    Invoke-Checked $generator @('--library', $nativeLibrary, '--config', (Join-Path $compatibility 'probe/uniffi.toml'), '--out-dir', $generated, '--no-format')
    Invoke-Checked 'dotnet' @('run', '--project', (Join-Path $compatibility 'runner/InteropProbe.csproj'), '--configuration', $Configuration, '--no-launch-profile')
}
finally {
    Pop-Location
}
