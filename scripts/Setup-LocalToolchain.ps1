$ErrorActionPreference = 'Stop'

$workspaceRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$toolRoot = Join-Path $workspaceRoot '.tools'
$rustupInit = Join-Path $toolRoot 'rustup-init.exe'
$cargoHome = Join-Path $toolRoot 'cargo'
$rustupHome = Join-Path $toolRoot 'rustup'
$cargoExecutable = Join-Path $cargoHome 'bin\cargo.exe'
$rustupExecutable = Join-Path $cargoHome 'bin\rustup.exe'

New-Item -ItemType Directory -Force -Path $toolRoot | Out-Null
$env:RUSTUP_HOME = $rustupHome
$env:CARGO_HOME = $cargoHome

if (-not (Test-Path -LiteralPath $cargoExecutable)) {
    if (-not (Test-Path -LiteralPath $rustupInit)) {
        Invoke-WebRequest -UseBasicParsing -Uri 'https://win.rustup.rs/x86_64' -OutFile $rustupInit
    }

    & $rustupInit -y --no-modify-path --profile minimal --default-host x86_64-pc-windows-msvc --default-toolchain stable
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
}

& $rustupExecutable component add rustfmt clippy
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

Push-Location $workspaceRoot
try {
    & npm.cmd install
    exit $LASTEXITCODE
}
finally {
    Pop-Location
}
