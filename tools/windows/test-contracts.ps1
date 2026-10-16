#Requires -Version 7.0
[CmdletBinding()]
param([switch]$SkipBuild)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$repository = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
if (!$SkipBuild) { & (Join-Path $PSScriptRoot 'build-bridge.ps1') }
Push-Location (Join-Path $repository 'apps/windows')
try {
    & dotnet run --project contracts/NativeContracts.csproj --configuration Release --no-launch-profile
    if ($LASTEXITCODE -ne 0) { throw "Production bridge contracts failed with exit code $LASTEXITCODE" }
}
finally { Pop-Location }
