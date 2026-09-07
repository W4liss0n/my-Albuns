. (Join-Path $PSScriptRoot 'Rust-Toolchain.ps1')

function Initialize-MyAlbunsToolchain {
    # Bypass an inherited PowerShell 7 module path before Windows PowerShell
    # autoload resolves its incompatible Microsoft.PowerShell.Utility module.
    $utilityModuleManifest = Join-Path `
        $PSHOME `
        'Modules\Microsoft.PowerShell.Utility\Microsoft.PowerShell.Utility.psd1'
    Import-Module `
        -Name $utilityModuleManifest `
        -Global `
        -ErrorAction Stop

    $script:WorkspaceRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
    $script:RustToolchain = Get-MyAlbunsRustToolchain -WorkspaceRoot $script:WorkspaceRoot
    $script:ToolRoot = Join-Path $script:WorkspaceRoot '.tools'
    $script:CargoHome = Join-Path $script:ToolRoot 'cargo'
    $script:RustupHome = Join-Path $script:ToolRoot 'rustup'
    $script:CargoExecutable = Join-Path $script:CargoHome 'bin\cargo.exe'
    $devCommand = 'C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\Common7\Tools\VsDevCmd.bat'
    if (-not (Test-Path -LiteralPath $devCommand)) {
        $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
        if (Test-Path -LiteralPath $vswhere) {
            $installation = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
            if ($LASTEXITCODE -eq 0 -and -not [string]::IsNullOrWhiteSpace($installation)) {
                $devCommand = Join-Path $installation.Trim() 'Common7\Tools\VsDevCmd.bat'
            }
        }
    }
    $cargoBin = Join-Path $script:CargoHome 'bin'

    if (-not (Test-Path -LiteralPath $script:CargoExecutable)) {
        throw 'The local Rust toolchain does not exist. Run npm run setup:local.'
    }
    if (-not (Test-Path -LiteralPath $devCommand)) {
        throw 'A Visual Studio installation with the x64 C++ tools was not found.'
    }
    if ($env:MYALBUNS_LOCAL_TOOLCHAIN_INITIALIZED -eq '1') {
        $env:RUSTUP_HOME = $script:RustupHome
        $env:CARGO_HOME = $script:CargoHome
        $env:RUSTUP_TOOLCHAIN = $script:RustToolchain
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
    $env:RUSTUP_TOOLCHAIN = $script:RustToolchain
    $env:PATH = "$cargoBin;$env:PATH"
    $env:MYALBUNS_LOCAL_TOOLCHAIN_INITIALIZED = '1'
}

function Resolve-MyAlbunsCargoTargetDirectory {
    if ([string]::IsNullOrWhiteSpace($env:CARGO_TARGET_DIR)) {
        return [System.IO.Path]::GetFullPath(
            (Join-Path $script:WorkspaceRoot 'target')
        )
    }
    if ([System.IO.Path]::IsPathRooted($env:CARGO_TARGET_DIR)) {
        return [System.IO.Path]::GetFullPath($env:CARGO_TARGET_DIR)
    }

    # Every repository script invokes Cargo with the workspace as its current
    # directory. Cargo resolves a relative CARGO_TARGET_DIR against that
    # current directory, so consumers of built executables must do the same.
    return [System.IO.Path]::GetFullPath(
        (Join-Path $script:WorkspaceRoot $env:CARGO_TARGET_DIR)
    )
}

function Resolve-MyAlbunsWorkspaceRelativePath {
    param(
        [Parameter(Mandatory = $true)]
        [string] $Path
    )

    $workspaceRoot = [System.IO.Path]::GetFullPath($script:WorkspaceRoot)
    $candidatePath = [System.IO.Path]::GetFullPath($Path)
    $workspaceVolume = [System.IO.Path]::GetPathRoot($workspaceRoot)
    $candidateVolume = [System.IO.Path]::GetPathRoot($candidatePath)
    if (-not [string]::Equals(
            $workspaceVolume,
            $candidateVolume,
            [System.StringComparison]::OrdinalIgnoreCase
        )) {
        return $null
    }

    $trimmedWorkspace = $workspaceRoot.TrimEnd([char[]] @('\', '/'))
    $workspaceUri = [System.Uri]::new(
        $trimmedWorkspace + [System.IO.Path]::DirectorySeparatorChar
    )
    $candidateUri = [System.Uri]::new($candidatePath)
    $relativePath = [System.Uri]::UnescapeDataString(
        $workspaceUri.MakeRelativeUri($candidateUri).ToString()
    ).Replace('/', '\')
    if ([System.IO.Path]::IsPathRooted($relativePath)) {
        return $null
    }

    $roundTrip = [System.IO.Path]::GetFullPath(
        (Join-Path $workspaceRoot $relativePath)
    )
    if (-not [string]::Equals(
            $roundTrip,
            $candidatePath,
            [System.StringComparison]::OrdinalIgnoreCase
        )) {
        return $null
    }

    return $relativePath.Replace('\', '/')
}
