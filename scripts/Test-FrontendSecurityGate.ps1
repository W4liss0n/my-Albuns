param(
    [string] $OutputPath,
    [ValidateRange(1, 8)]
    [int] $CargoBuildJobs = 1
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

. (Join-Path $PSScriptRoot 'Local-Toolchain.ps1')
Initialize-MyAlbunsToolchain
. (Join-Path $PSScriptRoot 'Evidence-BuildInputs.ps1')

if ($env:OS -ne 'Windows_NT') {
    throw 'The frontend security gate must run on Windows.'
}

if ([string]::IsNullOrWhiteSpace($OutputPath)) {
    $OutputPath = Join-Path `
        $script:WorkspaceRoot `
        'docs\research\artifacts\0016-frontend-security-gate.json'
}
elseif (-not [System.IO.Path]::IsPathRooted($OutputPath)) {
    $OutputPath = Join-Path $script:WorkspaceRoot $OutputPath
}
$OutputPath = [System.IO.Path]::GetFullPath($OutputPath)
$artifactDirectory = [System.IO.Path]::GetFullPath(
    (Join-Path $script:WorkspaceRoot 'docs\research\artifacts')
)
if (
    [System.IO.Path]::GetDirectoryName($OutputPath) -cne
        $artifactDirectory -or
    [System.IO.Path]::GetExtension($OutputPath) -cne '.json'
) {
    throw 'Frontend security evidence must be a JSON file in docs\research\artifacts.'
}

$targetDirectory = [System.IO.Path]::GetFullPath(
    (Join-Path $script:WorkspaceRoot 'target\frontend-security-gate')
)
$checks = [System.Collections.Generic.List[object]]::new()

function Assert-ExactStringSet {
    param(
        [Parameter(Mandatory = $true)][string[]] $Actual,
        [Parameter(Mandatory = $true)][string[]] $Expected,
        [Parameter(Mandatory = $true)][string] $Label
    )

    $actualSet = @($Actual | Sort-Object -Unique)
    $expectedSet = @($Expected | Sort-Object -Unique)
    $difference = @(
        Compare-Object `
            -ReferenceObject $expectedSet `
            -DifferenceObject $actualSet `
            -CaseSensitive
    )
    if ($difference.Count -gt 0) {
        throw "$Label differs: $($difference | Out-String)"
    }
}

function Invoke-FrontendSecurityCheck {
    param(
        [Parameter(Mandatory = $true)][string] $Name,
        [Parameter(Mandatory = $true)][string] $Executable,
        [Parameter(Mandatory = $true)][string[]] $Arguments
    )

    $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    & $Executable @Arguments
    $exitCode = $LASTEXITCODE
    $stopwatch.Stop()
    if ($exitCode -ne 0) {
        throw "Frontend security check '$Name' failed with exit code $exitCode."
    }
    $checks.Add([ordered]@{
        name = $Name
        passed = $true
        elapsedMs = [long] $stopwatch.ElapsedMilliseconds
    })
}

$tokens = $null
$parserErrors = $null
[void] [System.Management.Automation.Language.Parser]::ParseFile(
    $PSCommandPath,
    [ref] $tokens,
    [ref] $parserErrors
)
if (@($parserErrors).Count -gt 0) {
    $messages = @($parserErrors | ForEach-Object { $_.Message }) -join ' | '
    throw "The frontend security runner has parser errors: $messages"
}
$checks.Add([ordered]@{
    name = 'powershell-runner-ast'
    passed = $true
    elapsedMs = 0
})

$initialBuildInputs = Get-BuildInputState
if ($initialBuildInputs.dirty) {
    throw 'Frontend security evidence requires clean source and build inputs.'
}

$previousCargoTarget = [System.Environment]::GetEnvironmentVariable(
    'CARGO_TARGET_DIR',
    [System.EnvironmentVariableTarget]::Process
)
$previousCargoBuildJobs = [System.Environment]::GetEnvironmentVariable(
    'CARGO_BUILD_JOBS',
    [System.EnvironmentVariableTarget]::Process
)
$locationWasPushed = $false

try {
    [System.Environment]::SetEnvironmentVariable(
        'CARGO_TARGET_DIR',
        $targetDirectory,
        [System.EnvironmentVariableTarget]::Process
    )
    [System.Environment]::SetEnvironmentVariable(
        'CARGO_BUILD_JOBS',
        [string] $CargoBuildJobs,
        [System.EnvironmentVariableTarget]::Process
    )
    Push-Location $script:WorkspaceRoot
    $locationWasPushed = $true

    Invoke-FrontendSecurityCheck `
        -Name 'rust-project-capability-contract' `
        -Executable $script:CargoExecutable `
        -Arguments @(
            'test', '-p', 'myalbuns-desktop', '--lib',
            'tests::project_windows_receive_only_the_explicit_frontend_commands',
            '--', '--exact'
        )
    Invoke-FrontendSecurityCheck `
        -Name 'rust-global-capability-contract' `
        -Executable $script:CargoExecutable `
        -Arguments @(
            'test', '-p', 'myalbuns-desktop', '--lib',
            'tests::global_window_receives_only_logging_and_the_native_open_dialog',
            '--', '--exact'
        )
    Invoke-FrontendSecurityCheck `
        -Name 'rust-asset-scope-contract' `
        -Executable $script:CargoExecutable `
        -Arguments @(
            'test', '-p', 'myalbuns-desktop', '--lib',
            'tests::asset_protocol_serves_only_published_media_previews',
            '--', '--exact'
        )
    Invoke-FrontendSecurityCheck `
        -Name 'tauri-acl-compilation' `
        -Executable $script:CargoExecutable `
        -Arguments @('check', '-p', 'myalbuns-desktop')
    Invoke-FrontendSecurityCheck `
        -Name 'frontend-tauri-boundary' `
        -Executable 'npx.cmd' `
        -Arguments @('vitest', 'run', 'src/platform/tauriBoundary.test.ts')
    Invoke-FrontendSecurityCheck `
        -Name 'frontend-production-build' `
        -Executable 'npm.cmd' `
        -Arguments @('run', 'build')
}
finally {
    if ($locationWasPushed) {
        Pop-Location
    }
    [System.Environment]::SetEnvironmentVariable(
        'CARGO_TARGET_DIR',
        $previousCargoTarget,
        [System.EnvironmentVariableTarget]::Process
    )
    [System.Environment]::SetEnvironmentVariable(
        'CARGO_BUILD_JOBS',
        $previousCargoBuildJobs,
        [System.EnvironmentVariableTarget]::Process
    )
}

$capabilitySpecifications = @(
    [ordered]@{
        capabilityPath = 'src-tauri\capabilities\default.json'
        permissionPath = 'src-tauri\permissions\project-window.json'
        permissionIdentifier = 'project-window-commands'
        pluginPermissions = @()
    },
    [ordered]@{
        capabilityPath = 'src-tauri\capabilities\global-shell.json'
        permissionPath = 'src-tauri\permissions\global-shell.json'
        permissionIdentifier = 'global-shell-logging'
        pluginPermissions = @('dialog:allow-open')
    }
)
$capabilityEvidence = [System.Collections.Generic.List[object]]::new()
foreach ($specification in $capabilitySpecifications) {
    $capability = Get-Content `
        -LiteralPath (Join-Path $script:WorkspaceRoot $specification.capabilityPath) `
        -Raw |
        ConvertFrom-Json
    $permissionManifest = Get-Content `
        -LiteralPath (Join-Path $script:WorkspaceRoot $specification.permissionPath) `
        -Raw |
        ConvertFrom-Json
    $manifestPermissions = @($permissionManifest.permission)
    if ($manifestPermissions.Count -ne 1) {
        throw "Capability '$($capability.identifier)' must own one permission."
    }
    $permission = $manifestPermissions[0]
    if ($permission.identifier -cne $specification.permissionIdentifier) {
        throw "Capability '$($capability.identifier)' does not match its permission."
    }
    $expectedCapabilityPermissions = @(
        [string] $permission.identifier
        @($specification.pluginPermissions)
    )
    Assert-ExactStringSet `
        -Actual @($capability.permissions) `
        -Expected $expectedCapabilityPermissions `
        -Label "Capability '$($capability.identifier)' source permissions"
    $capabilityEvidence.Add([ordered]@{
        identifier = [string] $capability.identifier
        local = [bool] $capability.local
        windows = @($capability.windows)
        permissions = @($capability.permissions)
        permissionIdentifier = [string] $permission.identifier
        pluginPermissions = @($specification.pluginPermissions)
        commands = @($permission.commands.allow | Sort-Object)
    })
}
$grantedPermissions = @($capabilityEvidence | ForEach-Object { $_.permissions })
$genericFilesystemPermission = @(
    $grantedPermissions |
        Where-Object { $_ -is [string] -and $_.StartsWith('fs:') }
).Count -gt 0
$genericShellPermission = @(
    $grantedPermissions |
        Where-Object { $_ -is [string] -and $_.StartsWith('shell:') }
).Count -gt 0
if ($genericFilesystemPermission -or $genericShellPermission) {
    throw 'The compiled frontend boundary may not grant filesystem or shell plugins.'
}

$tauriConfig = Get-Content `
    -LiteralPath (Join-Path $script:WorkspaceRoot 'src-tauri\tauri.conf.json') `
    -Raw |
    ConvertFrom-Json
$assetScopes = @($tauriConfig.app.security.assetProtocol.scope)

$compiledCapabilityFile = Get-ChildItem `
    -Path (Join-Path $targetDirectory 'debug\build\myalbuns-desktop-*\out\capabilities.json') `
    -File |
    Sort-Object LastWriteTimeUtc -Descending |
    Select-Object -First 1
if ($null -eq $compiledCapabilityFile) {
    throw 'Tauri did not emit a compiled capability manifest.'
}
$compiledAclPath = Join-Path $compiledCapabilityFile.DirectoryName 'acl-manifests.json'
if (-not (Test-Path -LiteralPath $compiledAclPath -PathType Leaf)) {
    throw 'Tauri did not emit its compiled ACL manifest.'
}
$compiledCapabilities = Get-Content `
    -LiteralPath $compiledCapabilityFile.FullName `
    -Raw |
    ConvertFrom-Json
Assert-ExactStringSet `
    -Actual @($compiledCapabilities.PSObject.Properties.Name) `
    -Expected @($capabilityEvidence | ForEach-Object { $_.identifier }) `
    -Label 'Source and compiled capability identifiers'
foreach ($evidence in $capabilityEvidence) {
    $compiledCapabilityProperty =
        $compiledCapabilities.PSObject.Properties[$evidence.identifier]
    if ($null -eq $compiledCapabilityProperty) {
        throw "Compiled capability '$($evidence.identifier)' is missing."
    }
    $compiledCapability = $compiledCapabilityProperty.Value
    Assert-ExactStringSet `
        -Actual @($compiledCapability.permissions) `
        -Expected @($evidence.permissions) `
        -Label "Capability '$($evidence.identifier)' permissions"
    Assert-ExactStringSet `
        -Actual @($compiledCapability.windows) `
        -Expected @($evidence.windows) `
        -Label "Capability '$($evidence.identifier)' windows"
    if ($compiledCapability.local -ne $true) {
        throw "Compiled capability '$($evidence.identifier)' is not local."
    }
}

$compiledAcl = Get-Content -LiteralPath $compiledAclPath -Raw | ConvertFrom-Json
$appAclProperty = $compiledAcl.PSObject.Properties['__app-acl__']
if ($null -eq $appAclProperty) {
    throw 'The compiled Tauri ACL has no application command manifest.'
}
$appAcl = $appAclProperty.Value
Assert-ExactStringSet `
    -Actual @($appAcl.permissions.PSObject.Properties.Name) `
    -Expected @($capabilityEvidence | ForEach-Object { $_.permissionIdentifier }) `
    -Label 'Compiled application permission identifiers'
foreach ($evidence in $capabilityEvidence) {
    $compiledPermission = $appAcl.permissions.PSObject.Properties[
        $evidence.permissionIdentifier
    ].Value
    if (@($compiledPermission.commands.deny).Count -gt 0) {
        throw "Permission '$($evidence.permissionIdentifier)' unexpectedly denies commands."
    }
    Assert-ExactStringSet `
        -Actual @($compiledPermission.commands.allow) `
        -Expected @($evidence.commands) `
        -Label "Permission '$($evidence.permissionIdentifier)' commands"
}
$dialogAclProperty = $compiledAcl.PSObject.Properties['dialog']
if ($null -eq $dialogAclProperty) {
    throw 'The compiled Tauri ACL has no native dialog manifest.'
}
$dialogOpenPermission =
    $dialogAclProperty.Value.permissions.PSObject.Properties['allow-open'].Value
Assert-ExactStringSet `
    -Actual @($dialogOpenPermission.commands.allow) `
    -Expected @('open') `
    -Label 'Compiled native open-dialog commands'
if (@($dialogOpenPermission.commands.deny).Count -gt 0) {
    throw 'The native open-dialog permission unexpectedly denies commands.'
}

$package = Get-Content `
    -LiteralPath (Join-Path $script:WorkspaceRoot 'package.json') `
    -Raw |
    ConvertFrom-Json
$frontendTauriPackages = @(
    $package.dependencies.PSObject.Properties.Name |
        Where-Object { $_.StartsWith('@tauri-apps/') }
)
Assert-ExactStringSet `
    -Actual $frontendTauriPackages `
    -Expected @('@tauri-apps/api', '@tauri-apps/plugin-dialog') `
    -Label 'Frontend Tauri packages'

$desktopCargo = Get-Content `
    -LiteralPath (Join-Path $script:WorkspaceRoot 'src-tauri\Cargo.toml') `
    -Raw
if ($desktopCargo.Contains('tauri-plugin-fs')) {
    throw 'The desktop host must not depend on the generic filesystem plugin.'
}
if (-not $desktopCargo.Contains('tauri-plugin-shell')) {
    throw 'The backend-only sidecar adapter unexpectedly lost its shell dependency.'
}
if (-not $desktopCargo.Contains('tauri-plugin-dialog')) {
    throw 'The global host unexpectedly lost its native dialog dependency.'
}
$desktopHostSource = Get-Content `
    -LiteralPath (Join-Path $script:WorkspaceRoot 'src-tauri\src\lib.rs') `
    -Raw
if (-not $desktopHostSource.Contains('.plugin(tauri_plugin_shell::init())')) {
    throw 'The backend-only shell plugin is not registered by the desktop host.'
}
$globalHostSource = Get-Content `
    -LiteralPath (
        Join-Path $script:WorkspaceRoot 'src-tauri\src\global_process_spike.rs'
    ) `
    -Raw
if (-not $globalHostSource.Contains('.plugin(tauri_plugin_dialog::init())')) {
    throw 'The native dialog plugin is not registered by the global host.'
}
if ($desktopHostSource.Contains('.plugin(tauri_plugin_dialog::init())')) {
    throw 'The Project host must not register the global native dialog plugin.'
}
$imagingAdapterSource = Get-Content `
    -LiteralPath (
        Join-Path $script:WorkspaceRoot 'src-tauri\src\imaging_processor.rs'
    ) `
    -Raw
if (-not $imagingAdapterSource.Contains('.sidecar("myalbuns-imaging")')) {
    throw 'The Imaging adapter no longer launches the fixed packaged sidecar.'
}

$finalBuildInputs = Get-BuildInputState
if (
    $finalBuildInputs.dirty -or
    $finalBuildInputs.fileCount -ne $initialBuildInputs.fileCount -or
    $finalBuildInputs.digestSha256 -cne $initialBuildInputs.digestSha256
) {
    throw 'Frontend security checks changed source inputs or ran against a dirty state.'
}
$gitCommit = (& git -C $script:WorkspaceRoot rev-parse HEAD).Trim()
if ($LASTEXITCODE -ne 0 -or $gitCommit -notmatch '^[0-9a-f]{40}$') {
    throw 'Could not identify the frontend security source commit.'
}
$workingTreeStatus = @(& git -C $script:WorkspaceRoot status --short)
if ($LASTEXITCODE -ne 0) {
    throw 'Could not inspect the working tree after the frontend security gate.'
}

$report = [ordered]@{
    schemaVersion = 2
    suite = 'frontend_security_capabilities'
    collectedAtUtc = [DateTimeOffset]::UtcNow.ToString('o')
    gitCommit = $gitCommit
    sourceInputsDirty = $false
    platform = [ordered]@{
        operatingSystem = [System.Environment]::OSVersion.VersionString
        architecture = (
            [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
        )
        powerShellEdition = [string] $PSVersionTable.PSEdition
        powerShellVersion = [string] $PSVersionTable.PSVersion
    }
    build = [ordered]@{
        profile = 'dev-check-and-test'
        cargoBuildJobs = $CargoBuildJobs
        targetDirectory = 'target/frontend-security-gate'
        buildInputFileCount = $finalBuildInputs.fileCount
        buildInputDigestSha256 = $finalBuildInputs.digestSha256
        buildInputsDirty = $false
        workingTreeDirty = $workingTreeStatus.Count -gt 0
    }
    checks = @($checks)
    results = [ordered]@{
        capabilities = @($capabilityEvidence)
        assetScopes = @($assetScopes)
        compiledAppManifest = $true
        genericFilesystemPermission = $genericFilesystemPermission
        genericShellPermission = $genericShellPermission
        backendSidecarShellDependency = $true
        backendSidecarShellRegistration = $true
        fixedImagingSidecarInvocation = $true
        globalNativeDialogDependency = $true
        globalNativeDialogRegistration = $true
        nativeDialogCommands = @('open')
    }
    limits = [ordered]@{
        topologyProbeCommandsRemainTemporary = $true
        assetScopeUsesTemporaryMyAlbuns2Namespace = $true
        runtimeWebView2AndInstallerGate = $false
    }
}

$outputDirectory = Split-Path -Parent $OutputPath
New-Item -ItemType Directory -Force -Path $outputDirectory | Out-Null
$json = $report | ConvertTo-Json -Depth 12
[System.IO.File]::WriteAllText(
    $OutputPath,
    $json + [System.Environment]::NewLine,
    [System.Text.UTF8Encoding]::new($false)
)
Write-Output "Frontend security report: $OutputPath"
Write-Output $json
