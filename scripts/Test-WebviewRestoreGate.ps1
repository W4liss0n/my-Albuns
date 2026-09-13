param(
    [switch] $AllowVisibleWindows,
    [ValidateSet('restore', 'creation')]
    [string] $Scenario = 'restore'
)

$ErrorActionPreference = 'Stop'
if (-not $AllowVisibleWindows) {
    throw 'This native probe opens its own disposable window. Obtain authorization for the desktop and pass -AllowVisibleWindows.'
}

. (Join-Path $PSScriptRoot 'Local-Toolchain.ps1')
Initialize-MyAlbunsToolchain
$targetDirectory = Resolve-MyAlbunsCargoTargetDirectory
$example = 'probe_webview_' + $Scenario
$probeExecutable = Join-Path $targetDirectory ('debug/examples/' + $example + '.exe')
$runDirectory = Join-Path $script:WorkspaceRoot (
    '.scratch/webview-' + $Scenario + '-gate/' + (Get-Date -Format 'yyyyMMdd-HHmmss-ffff')
)
$null = New-Item -ItemType Directory -Path $runDirectory -Force
$buildLog = Join-Path $runDirectory 'build.log'
$stdoutLog = Join-Path $runDirectory 'stdout.log'
$stderrLog = Join-Path $runDirectory 'stderr.log'
$dataDirectory = Join-Path $runDirectory 'webview-data'

Push-Location $script:WorkspaceRoot
try {
    # Windows PowerShell turns redirected native stderr warnings into errors.
    # Capture both streams directly and use Cargo's exit code as the result.
    $build = Start-Process -FilePath $script:CargoExecutable `
        -ArgumentList @('build', '-p', 'myalbuns-desktop', '--example', $example, '--features', 'tauri/custom-protocol') `
        -WorkingDirectory $script:WorkspaceRoot -WindowStyle Hidden -PassThru -Wait `
        -RedirectStandardOutput (Join-Path $runDirectory 'build.stdout.log') `
        -RedirectStandardError $buildLog
    if ($build.ExitCode -ne 0) {
        throw "The native probe build failed. See $buildLog"
    }
    if (-not (Test-Path -LiteralPath $probeExecutable -PathType Leaf)) {
        throw "The native probe executable is missing: $probeExecutable"
    }

    # Cargo examples do not receive Tauri's application resource. The same
    # embedded manifest is required to load Common Controls v6 on Windows.
    & mt.exe -nologo -manifest resources/windows/myalbuns.manifest "-outputresource:$probeExecutable;#1"
    if ($LASTEXITCODE -ne 0) {
        throw 'The native probe manifest could not be embedded.'
    }

    $probe = Start-Process -FilePath $probeExecutable `
        -ArgumentList @(('"' + $dataDirectory + '"'), '--allow-visible-windows') `
        -WorkingDirectory (Split-Path -Parent $probeExecutable) `
        -WindowStyle Hidden -PassThru `
        -RedirectStandardOutput $stdoutLog -RedirectStandardError $stderrLog
    $null = $probe.Handle
    if (-not $probe.WaitForExit(30000)) {
        $probe.Kill()
        $probe.WaitForExit()
        throw "The disposable native probe exceeded 30 seconds. See $runDirectory"
    }
    Get-Content -LiteralPath $stdoutLog
    Get-Content -LiteralPath $stderrLog
    if ($probe.ExitCode -ne 0) {
        throw "The native $Scenario probe failed with exit code $($probe.ExitCode). See $runDirectory"
    }
    Write-Output "Native $Scenario evidence: $runDirectory"
}
finally {
    Pop-Location
}
