#Requires -Version 7.0
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$repository = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$nativeRoot = Join-Path $repository 'target/windows-native'
$vcpkgRoot = Join-Path $nativeRoot 'vcpkg'
$revision = '3cd21e48a75f67acfef289da52893ce79144aaa1'

function Invoke-Checked {
    param([string]$Executable, [string[]]$Arguments)
    & $Executable @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$Executable failed with exit code $LASTEXITCODE" }
}

if (!(Test-Path -LiteralPath $vcpkgRoot)) {
    Invoke-Checked git @('clone', '--filter=blob:none', '--no-checkout', 'https://github.com/microsoft/vcpkg.git', $vcpkgRoot)
    Invoke-Checked git @('-C', $vcpkgRoot, 'checkout', '--detach', $revision)
}
$actualRevision = & git -C $vcpkgRoot rev-parse HEAD
if ($LASTEXITCODE -ne 0 -or $actualRevision.Trim() -ne $revision) {
    throw "Expected vcpkg checkout $revision; refusing to replace another checkout."
}
if (!(Test-Path -LiteralPath (Join-Path $vcpkgRoot 'vcpkg.exe'))) {
    Invoke-Checked (Join-Path $vcpkgRoot 'bootstrap-vcpkg.bat') @('-disableMetrics')
}

# Hydrate the historical FFmpeg port before vcpkg starts with its sanitized
# build environment. A partial Git clone otherwise fetches missing port blobs
# from inside checkout-index, which can fail with some Windows Git wrappers.
Invoke-Checked git @('-C', $vcpkgRoot, 'archive', '--format=tar', "--output=$(Join-Path $nativeRoot 'ffmpeg-port.tar')", '53babdc6f3e070e3b1200e8fab15cae681739b3c')

# Preserve the existing platform's libvpx 1.17.0 rather than accepting vcpkg's
# older 1.16.0. Only generated tool sources below target/ are changed here.
$overlayRoot = Join-Path $nativeRoot 'overlay-ports'
$ffmpegOverlay = Join-Path $overlayRoot 'ffmpeg'
[IO.Directory]::CreateDirectory($ffmpegOverlay) | Out-Null
Invoke-Checked tar @('-xf', (Join-Path $nativeRoot 'ffmpeg-port.tar'), '-C', $ffmpegOverlay)
$ffmpegPortPath = Join-Path $ffmpegOverlay 'portfile.cmake'
$ffmpegPort = [IO.File]::ReadAllText($ffmpegPortPath)
$originalOptions = 'set(OPTIONS "--enable-pic --disable-doc --enable-runtime-cpudetect --disable-autodetect")'
$minimalOptions = 'set(OPTIONS "--disable-everything --disable-network --enable-pic --disable-doc --enable-runtime-cpudetect --disable-autodetect")'
if (!$ffmpegPort.Contains($originalOptions)) { throw 'Pinned FFmpeg options no longer match.' }
$ffmpegPort = $ffmpegPort.Replace($originalOptions, $minimalOptions)
$optionsLog = 'message(STATUS "Building Options: ${OPTIONS}")'
if (!$ffmpegPort.Contains($optionsLog)) { throw 'Pinned FFmpeg configuration anchor no longer matches.' }
$decoderOptions = 'string(APPEND OPTIONS " --enable-decoder=libvpx_vp8,libvpx_vp9 --enable-demuxer=matroska --enable-parser=vp8,vp9 --enable-protocol=file --enable-filter=fps --disable-x86asm")'
$ffmpegPort = $ffmpegPort.Replace($optionsLog, $decoderOptions + "`n" + $optionsLog)
[IO.File]::WriteAllText($ffmpegPortPath, $ffmpegPort)
$vpxOverlay = Join-Path $overlayRoot 'libvpx'
if (!(Test-Path -LiteralPath $vpxOverlay)) {
    [IO.Directory]::CreateDirectory($overlayRoot) | Out-Null
    Copy-Item -LiteralPath (Join-Path $vcpkgRoot 'ports/libvpx') -Destination $vpxOverlay -Recurse
}
$originalPort = [IO.File]::ReadAllText((Join-Path $vcpkgRoot 'ports/libvpx/portfile.cmake'))
$originalManifest = [IO.File]::ReadAllText((Join-Path $vcpkgRoot 'ports/libvpx/vcpkg.json'))
$port = $originalPort.Replace('07f5e352411d6c0be331706d1835ac89bafbeddcbbac5542b473323766e9e974f4f68b33590f2aa50a7d8d69468a642b508cbb0a7c49a82c9933b07820f9c9d9', 'be98d1b913575e538fefefe657c9944df85118fe2d0558361495a72c75ac6a2251048cb645fca49e60d6941c823a923f31d4ebc4de9b778008c267c42805b339')
if ($port -eq $originalPort) { throw 'Pinned libvpx port checksum patch no longer matches.' }
$manifest = $originalManifest.Replace('"version": "1.16.0"', '"version": "1.17.0"')
if ($manifest -eq $originalManifest) { throw 'Pinned libvpx port version patch no longer matches.' }
[IO.File]::WriteAllText((Join-Path $vpxOverlay 'portfile.cmake'), $port)
[IO.File]::WriteAllText((Join-Path $vpxOverlay 'vcpkg.json'), $manifest)

$configuration = Join-Path $PSScriptRoot 'native'
Invoke-Checked (Join-Path $vcpkgRoot 'vcpkg.exe') @(
    'install', '--triplet=x64-windows-memedock',
    "--x-manifest-root=$configuration",
    "--x-install-root=$(Join-Path $nativeRoot 'installed')",
    "--overlay-triplets=$(Join-Path $configuration 'triplets')",
    "--overlay-ports=$(Join-Path $configuration 'ports')",
    "--overlay-ports=$overlayRoot",
    '--disable-metrics'
)
