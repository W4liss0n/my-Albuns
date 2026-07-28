$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'Local-Toolchain.ps1')
Initialize-MyAlbunsToolchain

Push-Location $script:WorkspaceRoot
try {
    & $script:CargoExecutable build -p myalbuns-imaging
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }

    $source = (Resolve-Path (Join-Path $script:WorkspaceRoot 'target\debug\myalbuns-imaging.exe')).Path
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
