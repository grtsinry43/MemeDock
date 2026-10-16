#Requires -Version 7.0
[CmdletBinding()]
param([switch]$SkipMedia, [switch]$SkipBridge)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$repository = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
& (Join-Path $PSScriptRoot 'check-environment.ps1') -RequireWinUI
if (!$SkipBridge) { & (Join-Path $PSScriptRoot 'build-bridge.ps1') -SkipMedia:$SkipMedia }
Push-Location (Join-Path $repository 'apps/windows')
try {
    & dotnet build app/MemeDock.Windows.csproj --configuration Release -p:RestoreLockedMode=true
    if ($LASTEXITCODE -ne 0) { throw "WinUI build failed with exit code $LASTEXITCODE" }
}
finally { Pop-Location }
