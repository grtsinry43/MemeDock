#Requires -Version 7.0
[CmdletBinding()]
param([switch]$SkipMedia, [switch]$VerifyRust)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$repository = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$nativeRoot = Join-Path $repository 'target/windows-native'
$prefix = Join-Path $nativeRoot 'installed/x64-windows-memedock'
$hostPrefix = Join-Path $nativeRoot 'installed/x64-windows'
$buildRoot = Join-Path $nativeRoot 'cargo'
$compatBuild = Join-Path $repository 'target/windows-compat'

function Invoke-Checked {
    param([string]$Executable, [string[]]$Arguments)
    & $Executable @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$Executable failed with exit code $LASTEXITCODE" }
}

if (!$SkipMedia) { & (Join-Path $PSScriptRoot 'prepare-media.ps1') }
& (Join-Path $PSScriptRoot 'prepare-bindgen.ps1')

$pkgconf = Join-Path $hostPrefix 'tools/pkgconf/pkgconf.exe'
if (!(Test-Path -LiteralPath $pkgconf)) { throw 'Build the media dependencies first; pkgconf is missing.' }
$clangDirectory = if ($env:LIBCLANG_PATH) { $env:LIBCLANG_PATH } else { 'C:/Program Files/LLVM/bin' }
if (!(Test-Path -LiteralPath (Join-Path $clangDirectory 'libclang.dll'))) {
    throw 'Set LIBCLANG_PATH to an LLVM bin directory containing libclang.dll.'
}

$environmentOverrides = @{
    FFMPEG_DIR = $prefix
    PKG_CONFIG = $pkgconf
    PKG_CONFIG_PATH = (Join-Path $prefix 'lib/pkgconfig')
    PKG_CONFIG_LIBDIR = (Join-Path $prefix 'lib/pkgconfig')
    LIBCLANG_PATH = $clangDirectory
    PATH = "$(Join-Path $prefix 'bin');$env:PATH"
}
$savedEnvironment = @{}
foreach ($name in $environmentOverrides.Keys) {
    $savedEnvironment[$name] = [Environment]::GetEnvironmentVariable($name, 'Process')
    [Environment]::SetEnvironmentVariable($name, $environmentOverrides[$name], 'Process')
}

Push-Location $repository
try {
    Invoke-Checked cargo @('build', '-p', 'memedock-ffi', '--release', '--locked', '--target', 'x86_64-pc-windows-msvc', '--target-dir', $buildRoot)
    if ($VerifyRust) {
        $checks = @('-p', 'memedock-domain', '-p', 'memedock-storage', '-p', 'memedock-core', '-p', 'memedock-ffi', '--release', '--locked', '--target', 'x86_64-pc-windows-msvc', '--target-dir', $buildRoot)
        Invoke-Checked cargo (@('clippy', '--all-targets') + $checks + @('--', '-D', 'warnings'))
        Invoke-Checked cargo (@('test') + $checks)
    }
    Invoke-Checked cargo @('build', '--manifest-path', 'apps/windows/compatibility/Cargo.toml', '-p', 'memedock-windows-bindgen', '--locked', '--target', 'x86_64-pc-windows-msvc', '--target-dir', $compatBuild)
    $runtime = Join-Path $nativeRoot 'runtime'
    $generated = Join-Path $nativeRoot 'generated'
    [IO.Directory]::CreateDirectory($runtime) | Out-Null
    [IO.Directory]::CreateDirectory($generated) | Out-Null
    $library = Join-Path $buildRoot 'x86_64-pc-windows-msvc/release/memedock_ffi.dll'
    Copy-Item -LiteralPath $library -Destination $runtime
    Get-ChildItem -LiteralPath (Join-Path $prefix 'bin') -Filter '*.dll' | Copy-Item -Destination $runtime
    $licenses = Join-Path $runtime 'licenses'
    foreach ($dependency in @('ffmpeg', 'libvpx', 'libwebp', 'rlottie-telegram')) {
        $destination = Join-Path $licenses $dependency
        [IO.Directory]::CreateDirectory($destination) | Out-Null
        Copy-Item -LiteralPath (Join-Path $prefix "share/$dependency/copyright") -Destination $destination
    }
    $rustLicenseDirectory = Join-Path $licenses 'rlottie-rs'
    [IO.Directory]::CreateDirectory($rustLicenseDirectory) | Out-Null
    Copy-Item -LiteralPath (Join-Path $repository 'third-party/rlottie/LICENSE') -Destination $rustLicenseDirectory
    $generator = Join-Path $compatBuild 'x86_64-pc-windows-msvc/debug/memedock-windows-bindgen.exe'
    Invoke-Checked $generator @('--library', $library, '--config', (Join-Path $repository 'apps/windows/bridge/uniffi.toml'), '--out-dir', $generated, '--no-format')
    Push-Location (Join-Path $repository 'apps/windows')
    try {
        Invoke-Checked dotnet @('build', 'bridge/MemeDock.Bridge.csproj', '--configuration', 'Release')
    }
    finally { Pop-Location }
}
finally {
    Pop-Location
    foreach ($name in $savedEnvironment.Keys) {
        [Environment]::SetEnvironmentVariable($name, $savedEnvironment[$name], 'Process')
    }
}
