#Requires -Version 7.0
[CmdletBinding()]
param([switch]$RequireWinUI)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

if (![OperatingSystem]::IsWindows()) {
    throw 'Run this checker on Windows with PowerShell 7.'
}

$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
if (!(Test-Path -LiteralPath $vswhere)) {
    throw 'Visual Studio Installer/vswhere.exe is missing.'
}

$instances = @((& $vswhere -all -products '*' -format json | ConvertFrom-Json))
if ($LASTEXITCODE -ne 0) { throw 'vswhere could not enumerate Visual Studio.' }
foreach ($instance in $instances) {
    Write-Host "$($instance.displayName) $($instance.installationVersion): $($instance.installationPath)"
}

$components = [ordered]@{
    'Microsoft.VisualStudio.Workload.ManagedDesktop' = '.NET desktop development'
    'Microsoft.VisualStudio.Component.VC.Tools.x86.x64' = 'MSVC x64/x86 build tools'
    'Microsoft.VisualStudio.Workload.Universal' = 'WinUI application development'
    'Microsoft.VisualStudio.ComponentGroup.WindowsAppSDK.Cs' = 'Windows App SDK C# templates'
    'Microsoft.VisualStudio.Component.Windows11SDK.26100' = 'Windows SDK 10.0.26100'
}
$missingWinUI = @()
$missingNative = @()
foreach ($component in $components.GetEnumerator()) {
    $paths = @(& $vswhere -all -products '*' -requires $component.Key -property installationPath)
    if ($LASTEXITCODE -ne 0) { throw "vswhere failed checking $($component.Key)." }
    $installed = $paths.Count -gt 0
    Write-Host ("{0}: {1}" -f $component.Value, $(if ($installed) { 'installed' } else { 'MISSING' }))
    if (!$installed) {
        if ($component.Key -in @('Microsoft.VisualStudio.Workload.ManagedDesktop', 'Microsoft.VisualStudio.Component.VC.Tools.x86.x64')) {
            $missingNative += $component.Key
        }
        else { $missingWinUI += $component.Key }
    }
}

if (!(Get-Command dotnet -ErrorAction SilentlyContinue)) { throw '.NET SDK is missing.' }
$sdkLines = @(& dotnet --list-sdks)
if ($LASTEXITCODE -ne 0) { throw 'Cannot query .NET SDKs.' }
$sdkLines | ForEach-Object { Write-Host $_ }
if (!($sdkLines | Where-Object { $_ -match '^8\.0\.4\d{2}\s' })) {
    throw 'The compatibility runner requires a .NET 8.0.4xx SDK (apps/windows/global.json).'
}
if (!(Get-Command cargo -ErrorAction SilentlyContinue)) { throw 'Cargo is missing.' }
if ($missingNative.Count -gt 0) { throw 'Install the missing desktop/MSVC components before building Rust.' }

if ($missingWinUI.Count -gt 0) {
    Write-Warning 'WinUI tools are incomplete. Import apps/windows/.vsconfig in Visual Studio Installer. The console interop probe can run independently.'
    if ($RequireWinUI) { throw 'WinUI prerequisites are missing.' }
}
