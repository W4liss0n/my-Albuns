param(
    [ValidateSet('debug', 'release')]
    [string] $Profile = 'debug'
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'Local-Toolchain.ps1')
Initialize-MyAlbunsToolchain

Push-Location $script:WorkspaceRoot
try {
    $buildArguments = @('build', '-p', 'myalbuns-imaging')
    if ($Profile -eq 'release') {
        $buildArguments += '--release'
    }
    & $script:CargoExecutable @buildArguments
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }

    $targetDirectory = if ([string]::IsNullOrWhiteSpace($env:CARGO_TARGET_DIR)) {
        Join-Path $script:WorkspaceRoot 'target'
    }
    elseif ([System.IO.Path]::IsPathRooted($env:CARGO_TARGET_DIR)) {
        [System.IO.Path]::GetFullPath($env:CARGO_TARGET_DIR)
    }
    else {
        [System.IO.Path]::GetFullPath((Join-Path $script:WorkspaceRoot $env:CARGO_TARGET_DIR))
    }
    $source = (
        Resolve-Path (
            Join-Path $targetDirectory "$Profile\myalbuns-imaging.exe"
        )
    ).Path
    $binaryDirectory = Join-Path $script:WorkspaceRoot 'src-tauri\binaries'
    $destination = Join-Path $binaryDirectory 'myalbuns-imaging-x86_64-pc-windows-msvc.exe'

    if (-not $source.StartsWith($script:WorkspaceRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "Origem inesperada do sidecar: $source"
    }
    if (-not $destination.StartsWith($script:WorkspaceRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
        throw "Destino inesperado do sidecar: $destination"
    }

    New-Item -ItemType Directory -Force -Path $binaryDirectory | Out-Null
    Copy-Item -LiteralPath $source -Destination $destination -Force
}
finally {
    Pop-Location
}
