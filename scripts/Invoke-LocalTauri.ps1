param(
    [ValidateSet('dev', 'build')]
    [string] $Action = 'dev',

    [string] $ProjectPath,

    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]] $TauriArguments
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'Local-Toolchain.ps1')
. (Join-Path $PSScriptRoot 'Local-TauriBuild.ps1')
Initialize-MyAlbunsToolchain

Push-Location $script:WorkspaceRoot
try {
    if ($Action -eq 'dev') {
        & (Join-Path $PSScriptRoot 'Prepare-DevLauncher.ps1')
        if ($LASTEXITCODE -ne 0) {
            exit $LASTEXITCODE
        }

        $launcher = Join-Path `
            (Resolve-MyAlbunsCargoTargetDirectory) `
            'debug\myalbuns-dev.exe'
        $previousWorkspace = $env:MYALBUNS_DEV_WORKSPACE_ROOT
        $previousProject = $env:MYALBUNS_DEV_PROJECT_PATH
        try {
            $env:MYALBUNS_DEV_WORKSPACE_ROOT = $script:WorkspaceRoot
            if ($ProjectPath) {
                $env:MYALBUNS_DEV_PROJECT_PATH = (
                    [System.IO.Path]::GetFullPath($ProjectPath)
                )
            }
            else {
                Remove-Item Env:MYALBUNS_DEV_PROJECT_PATH -ErrorAction SilentlyContinue
            }
            & $launcher @TauriArguments
            exit $LASTEXITCODE
        }
        finally {
            $env:MYALBUNS_DEV_WORKSPACE_ROOT = $previousWorkspace
            $env:MYALBUNS_DEV_PROJECT_PATH = $previousProject
        }
    }

    if ($ProjectPath) {
        throw '-ProjectPath is available only for the development launcher.'
    }
    Invoke-MyAlbunsTauriBuild -TauriArguments $TauriArguments
    exit $LASTEXITCODE
}
finally {
    Pop-Location
}
