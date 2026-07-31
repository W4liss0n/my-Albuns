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
        -Name 'rust-capability-contract' `
        -Executable $script:CargoExecutable `
        -Arguments @(
            'test', '-p', 'myalbuns-desktop', '--lib',
            'tests::project_windows_receive_only_the_explicit_frontend_commands',
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

$capabilityPath = Join-Path `
    $script:WorkspaceRoot `
    'src-tauri\capabilities\default.json'
$capability = Get-Content -LiteralPath $capabilityPath -Raw | ConvertFrom-Json
if ($capability.local -ne $true) {
    throw 'The project-window capability must be explicitly local.'
}
if ($capability.PSObject.Properties.Name -ccontains 'remote') {
    throw 'The project-window capability must not authorize remote origins.'
}
Assert-ExactStringSet `
    -Actual @($capability.windows) `
    -Expected @('main', 'project-b') `
    -Label 'Capability window labels'

$grantedPermissions = @($capability.permissions)
if (
    @($grantedPermissions | Sort-Object -Unique).Count -ne
        $grantedPermissions.Count
) {
    throw 'The capability repeats a permission identifier.'
}
foreach ($permission in $grantedPermissions) {
    if ($permission -isnot [string]) {
        throw 'The frontend does not need scoped plugin permission objects.'
    }
    if (
        $permission.StartsWith('core:') -or
        $permission.StartsWith('fs:') -or
        $permission.StartsWith('shell:')
    ) {
        throw "Unexpected frontend permission: $permission"
    }
}
Assert-ExactStringSet `
    -Actual $grantedPermissions `
    -Expected @('project-window-commands') `
    -Label 'Capability permissions'

$permissionManifest = Get-Content `
    -LiteralPath (
        Join-Path `
            $script:WorkspaceRoot `
            'src-tauri\permissions\project-window.json'
    ) `
    -Raw |
    ConvertFrom-Json
$permissionEntries = @(
    $permissionManifest.permission |
        Where-Object { $_.identifier -ceq 'project-window-commands' }
)
if ($permissionEntries.Count -ne 1) {
    throw 'The project-window permission manifest must define one authoritative entry.'
}
$projectWindowPermission = $permissionEntries[0]
$allowedCommands = @($projectWindowPermission.commands.allow)
if ($allowedCommands.Count -eq 0) {
    throw 'The project-window permission command allow-list is empty.'
}
if (
    @($allowedCommands | Sort-Object -Unique).Count -ne
        $allowedCommands.Count
) {
    throw 'The project-window permission repeats an application command.'
}
if (@($projectWindowPermission.commands.deny).Count -gt 0) {
    throw 'The project-window permission must not mix allow and deny command lists.'
}
if (@($allowedCommands | Where-Object { $_.Contains(':') }).Count -gt 0) {
    throw 'The project-window permission may grant only application commands.'
}

$frontendCommands = @(
    Get-ChildItem `
        -LiteralPath (Join-Path $script:WorkspaceRoot 'src\platform') `
        -Filter '*.ts' `
        -File |
        Where-Object { $_.Name -notlike '*.test.ts' } |
        ForEach-Object {
            $source = Get-Content -LiteralPath $_.FullName -Raw
            [regex]::Matches(
                $source,
                '\binvoke(?:<[^>]+>)?\(\s*["''](?<command>[^"'']+)["'']'
            ) | ForEach-Object { $_.Groups['command'].Value }
        }
)
Assert-ExactStringSet `
    -Actual $allowedCommands `
    -Expected $frontendCommands `
    -Label 'Permission and frontend command lists'

$desktopSource = Get-Content `
    -LiteralPath (Join-Path $script:WorkspaceRoot 'src-tauri\src\lib.rs') `
    -Raw
$handlerMatch = [regex]::Match(
    $desktopSource,
    '\.invoke_handler\(tauri::generate_handler!\[(?<commands>[\s\S]*?)\]\)'
)
if (-not $handlerMatch.Success) {
    throw 'Could not locate the desktop invoke handler.'
}
$registeredCommands = @(
    [regex]::Matches(
        $handlerMatch.Groups['commands'].Value,
        '\b[a-z][a-z0-9_]*\b'
    ) | ForEach-Object { $_.Value }
)
Assert-ExactStringSet `
    -Actual $registeredCommands `
    -Expected $frontendCommands `
    -Label 'Runtime handler and frontend command lists'

$tauriConfig = Get-Content `
    -LiteralPath (Join-Path $script:WorkspaceRoot 'src-tauri\tauri.conf.json') `
    -Raw |
    ConvertFrom-Json
$assetScopes = @($tauriConfig.app.security.assetProtocol.scope)
Assert-ExactStringSet `
    -Actual $assetScopes `
    -Expected @(
        '$LOCALDATA/MyAlbuns2/Cache/*/Media/*.jpg',
        '$LOCALDATA/MyAlbuns2/Cache/*/Media/*.png'
    ) `
    -Label 'Asset protocol scopes'
if (@($assetScopes | Where-Object { $_.Contains('**') }).Count -gt 0) {
    throw 'The asset protocol must not expose a recursive Cache scope.'
}

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
$compiledCapability = $compiledCapabilities.default
Assert-ExactStringSet `
    -Actual @($compiledCapability.permissions) `
    -Expected $grantedPermissions `
    -Label 'Source and compiled capability permissions'
Assert-ExactStringSet `
    -Actual @($compiledCapability.windows) `
    -Expected @($capability.windows) `
    -Label 'Source and compiled capability windows'
if ($compiledCapability.local -ne $true) {
    throw 'The compiled project-window capability is not local.'
}

$compiledAcl = Get-Content -LiteralPath $compiledAclPath -Raw | ConvertFrom-Json
$appAclProperty = $compiledAcl.PSObject.Properties['__app-acl__']
if ($null -eq $appAclProperty) {
    throw 'The compiled Tauri ACL has no application command manifest.'
}
$appAcl = $appAclProperty.Value
Assert-ExactStringSet `
    -Actual @($appAcl.permissions.PSObject.Properties.Name) `
    -Expected @('project-window-commands') `
    -Label 'Compiled application permission identifiers'
$compiledProjectWindowPermission =
    $appAcl.permissions.PSObject.Properties['project-window-commands'].Value
$compiledAllowedCommands = @(
    $compiledProjectWindowPermission.commands.allow
)
if (@($compiledProjectWindowPermission.commands.deny).Count -gt 0) {
    throw 'The compiled application permission unexpectedly denies commands.'
}
Assert-ExactStringSet `
    -Actual $compiledAllowedCommands `
    -Expected $allowedCommands `
    -Label 'Compiled AppManifest and capability commands'

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
    -Expected @('@tauri-apps/api') `
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
    schemaVersion = 1
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
        capabilityIdentifier = [string] $capability.identifier
        local = [bool] $capability.local
        windows = @($capability.windows)
        commandCount = $allowedCommands.Count
        commands = @($allowedCommands | Sort-Object)
        permissions = @($grantedPermissions)
        assetScopes = @($assetScopes)
        compiledAppManifest = $true
        genericFilesystemPermission = $false
        genericShellPermission = $false
        backendSidecarShellDependency = $true
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
