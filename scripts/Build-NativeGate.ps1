param([string] $OutputPath = '.tools\native-gate-build.json')
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'Local-Toolchain.ps1')
. (Join-Path $PSScriptRoot 'Local-TauriBuild.ps1')
. (Join-Path $PSScriptRoot 'Native-GateBuild.ps1')
Initialize-MyAlbunsToolchain
if (-not [IO.Path]::IsPathRooted($OutputPath)) {
    $OutputPath = Join-Path $script:WorkspaceRoot $OutputPath
}
$sourceBefore = Get-GateSourceSnapshot -WorkspaceRoot $script:WorkspaceRoot -EvidencePath $OutputPath
if ($sourceBefore.sourceInputsDirty) {
    throw 'Commit source changes before preparing a reusable native test build.'
}
Push-Location $script:WorkspaceRoot
try {
    Invoke-MyAlbunsTauriBuild -TauriArguments @('--debug', '--no-bundle')
    if ($LASTEXITCODE -ne 0) { throw 'The native test application build failed.' }
    & $script:CargoExecutable build -p myalbuns-desktop --example prepare_focused_owned_dialog_fixtures
    if ($LASTEXITCODE -ne 0) { throw 'The native fixture builder failed.' }
    $target = Resolve-MyAlbunsCargoTargetDirectory
    $applicationPath = Join-Path $target 'debug\myalbuns-desktop.exe'
    $sourceAfter = Get-GateSourceSnapshot -WorkspaceRoot $script:WorkspaceRoot -EvidencePath $OutputPath
    if (Test-GateSourceSnapshotsDirty -Before $sourceBefore -After $sourceAfter) {
        throw 'The source changed while preparing the native test build.'
    }
    $application = Get-NativeGateArtifact -Path $applicationPath
    $application.buildMode = 'tauri-debug-custom-protocol'
    $application.relativePath = Resolve-MyAlbunsWorkspaceRelativePath -Path $applicationPath
    $manifest = [ordered]@{
        schemaVersion = 1
        buildMode = 'tauri-debug-custom-protocol'
        gitCommit = $sourceBefore.gitCommit
        sourceInputsDirty = $false
        application = $application
        fixture = Get-NativeGateArtifact -Path (Join-Path $target 'debug\examples\prepare_focused_owned_dialog_fixtures.exe')
        processor = Get-NativeGateArtifact -Path (Join-Path $target 'debug\myalbuns-imaging.exe')
    }
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $OutputPath) | Out-Null
    [IO.File]::WriteAllText($OutputPath, ($manifest | ConvertTo-Json -Depth 6) + [Environment]::NewLine, [Text.UTF8Encoding]::new($false))
    Write-Output "Reusable native test build: $OutputPath"
}
finally { Pop-Location }
