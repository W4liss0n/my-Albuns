function Initialize-MyAlbunsToolchain {
    $script:WorkspaceRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
    $script:ToolRoot = Join-Path $script:WorkspaceRoot '.tools'
    $script:CargoHome = Join-Path $script:ToolRoot 'cargo'
    $script:RustupHome = Join-Path $script:ToolRoot 'rustup'
    $script:CargoExecutable = Join-Path $script:CargoHome 'bin\cargo.exe'
    $devCommand = 'C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\Common7\Tools\VsDevCmd.bat'
    $cargoBin = Join-Path $script:CargoHome 'bin'

    if (-not (Test-Path -LiteralPath $script:CargoExecutable)) {
        throw 'The local Rust toolchain does not exist. Run npm run setup:local.'
    }
    if (-not (Test-Path -LiteralPath $devCommand)) {
        throw 'Microsoft Visual Studio Build Tools 2022 was not found.'
    }
    if ($env:MYALBUNS_LOCAL_TOOLCHAIN_INITIALIZED -eq '1') {
        $env:RUSTUP_HOME = $script:RustupHome
        $env:CARGO_HOME = $script:CargoHome
        if (
            $cargoBin -notin @(
                $env:PATH -split ';' |
                    Where-Object { -not [string]::IsNullOrWhiteSpace($_) }
            )
        ) {
            $env:PATH = "$cargoBin;$env:PATH"
        }
        return
    }

    $devEnvironment = & cmd.exe /d /s /c "`"$devCommand`" -arch=x64 -host_arch=x64 >nul && set"
    foreach ($entry in $devEnvironment) {
        if ($entry -match '^([^=]+)=(.*)$') {
            Set-Item -Path "Env:$($matches[1])" -Value $matches[2]
        }
    }

    $env:RUSTUP_HOME = $script:RustupHome
    $env:CARGO_HOME = $script:CargoHome
    $env:PATH = "$cargoBin;$env:PATH"
    $env:MYALBUNS_LOCAL_TOOLCHAIN_INITIALIZED = '1'
}
