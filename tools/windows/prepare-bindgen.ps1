#Requires -Version 7.0
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repository = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$source = Join-Path $repository 'target/windows-compat/generator-source'
$revision = 'cf0aa7a8fb59358a846708730a58a02bf206ac05'
$origin = 'https://github.com/dennisameling/uniffi-bindgen-cs.git'

function Invoke-Git {
    param([string[]]$Arguments)
    & git @Arguments
    if ($LASTEXITCODE -ne 0) { throw "git failed with exit code $LASTEXITCODE" }
}

if (!(Test-Path -LiteralPath $source)) {
    Invoke-Git @('clone', '--no-checkout', $origin, $source)
    Invoke-Git @('-C', $source, 'checkout', '--detach', $revision)
}
$actualRevision = & git -C $source rev-parse HEAD
if ($LASTEXITCODE -ne 0 -or $actualRevision.Trim() -ne $revision) {
    throw "Generator checkout must be at $revision; refusing to replace an existing checkout."
}
$changes = @(& git -C $source status --porcelain)
if ($LASTEXITCODE -ne 0) { throw 'Cannot inspect the generator checkout.' }
if ($changes | Where-Object { $_ -ne ' M bindgen/Cargo.toml' }) {
    throw 'Unexpected changes in the generated tool checkout; refusing to modify it.'
}

# 0.32.2 exposes toml 1.x in BindingGenerator. The PR still constrains toml
# below 0.10. Patch only this build-tool dependency; preserve upstream code.
$originalLines = @(& git -C $source show 'HEAD:bindgen/Cargo.toml')
if ($LASTEXITCODE -ne 0) { throw 'Cannot read the pinned generator manifest.' }
$original = ($originalLines -join "`n").TrimEnd("`r", "`n")
$expected = $original.Replace('toml = ">=0.8, <0.10"', 'toml = "=1.1.8"')
if ($expected -eq $original) { throw 'The pinned TOML dependency patch no longer matches.' }
$manifest = Join-Path $source 'bindgen/Cargo.toml'
$current = [IO.File]::ReadAllText($manifest).Replace("`r`n", "`n").TrimEnd("`r", "`n")
if ($current -ne $original -and $current -ne $expected) {
    throw 'Unexpected manifest edits in the generator checkout.'
}
if ($current -ne $expected) {
    [IO.File]::WriteAllText($manifest, $expected, [Text.UTF8Encoding]::new($false))
}
Write-Output "Prepared C# generator $revision with the TOML 1.x compatibility patch."
