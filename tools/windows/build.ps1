#Requires -Version 7.0
[CmdletBinding()]
param(
    [switch]$SkipMedia, [switch]$SkipBridge, [switch]$SkipEnvironmentCheck,
    [ValidateSet('Debug', 'Release')][string]$Configuration = 'Release'
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$repository = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
# check-environment.ps1 inspects a Visual Studio installation, which only exists on
# a developer workstation; CI installs and verifies its own toolchain instead.
if (!$SkipEnvironmentCheck) { & (Join-Path $PSScriptRoot 'check-environment.ps1') -RequireWinUI }
if (!$SkipBridge) { & (Join-Path $PSScriptRoot 'build-bridge.ps1') -SkipMedia:$SkipMedia -Configuration $Configuration }
Push-Location (Join-Path $repository 'apps/windows')
try {
    & dotnet build app/MemeDock.Windows.csproj --configuration $Configuration -p:RestoreLockedMode=true
    if ($LASTEXITCODE -ne 0) { throw "WinUI build failed with exit code $LASTEXITCODE" }
}
finally { Pop-Location }
