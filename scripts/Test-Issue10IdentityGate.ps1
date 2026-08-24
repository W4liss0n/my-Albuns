param(
    [string] $OutputPath,
    [string] $UncRoot,
    [string] $DriveLetter
)

$ErrorActionPreference = 'Stop'

Import-Module Microsoft.PowerShell.Utility
. (Join-Path $PSScriptRoot 'Local-Toolchain.ps1')
. (Join-Path $PSScriptRoot 'Gate-SourceProvenance.ps1')
. (Join-Path $PSScriptRoot 'Gate-ScratchDirectory.ps1')
. (Join-Path $PSScriptRoot 'Gate-WindowsProcessArgument.ps1')
Initialize-MyAlbunsToolchain

if (-not $IsWindows -and $env:OS -ne 'Windows_NT') {
    throw 'The issue 10 Identidade gate must run on Windows.'
}

if ([string]::IsNullOrWhiteSpace($OutputPath)) {
    $OutputPath = Join-Path `
        $script:WorkspaceRoot `
        'docs\research\artifacts\0035-issue-10-identity-gate.json'
}
elseif (-not [System.IO.Path]::IsPathRooted($OutputPath)) {
    $OutputPath = Join-Path $script:WorkspaceRoot $OutputPath
}
$OutputPath = [System.IO.Path]::GetFullPath($OutputPath)
$sourceSnapshotBefore = Get-GateSourceSnapshot `
    -WorkspaceRoot $script:WorkspaceRoot `
    -EvidencePath $OutputPath

$scratchRoot = [System.IO.Path]::GetFullPath(
    (Join-Path $script:WorkspaceRoot '.scratch\cargo-target-tests\issue10-identity-gate')
)
$runRoot = [System.IO.Path]::GetFullPath(
    (Join-Path $scratchRoot "run-$PID-$([DateTime]::UtcNow.Ticks)")
)
if (-not $runRoot.StartsWith(
        $scratchRoot + [System.IO.Path]::DirectorySeparatorChar,
        [System.StringComparison]::OrdinalIgnoreCase
    )) {
    throw 'The issue 10 fixture escaped its ignored scratch root.'
}
New-Item -ItemType Directory -Force -Path $runRoot | Out-Null

if ([string]::IsNullOrWhiteSpace($UncRoot)) {
    $volumeRoot = [System.IO.Path]::GetPathRoot($runRoot)
    if ($volumeRoot -notmatch '^[A-Za-z]:\\$') {
        throw 'The default UNC fixture requires a drive-letter volume.'
    }
    $volumeLetter = $volumeRoot.Substring(0, 1)
    $relativeRunRoot = $runRoot.Substring($volumeRoot.Length)
    $UncRoot = "\\127.0.0.1\$volumeLetter`$\$relativeRunRoot"
}
$UncRoot = $UncRoot.TrimEnd('\')

if ([string]::IsNullOrWhiteSpace($DriveLetter)) {
    $usedLetters = @(
        [System.IO.DriveInfo]::GetDrives() |
            ForEach-Object { $_.Name.Substring(0, 1).ToUpperInvariant() }
    )
    $DriveLetter = @('R', 'Q', 'P', 'O', 'N', 'M') |
        Where-Object { $usedLetters -notcontains $_ } |
        Select-Object -First 1
}
$DriveLetter = $DriveLetter.TrimEnd(':').ToUpperInvariant()
if ($DriveLetter -notmatch '^[A-Z]$') {
    throw 'DriveLetter must be one unused letter.'
}
$mappedDrive = "$DriveLetter`:"

$gateExecutable = Join-Path `
    $script:WorkspaceRoot `
    'target\debug\examples\issue10_identity_gate.exe'
$leaseRoot = Join-Path $runRoot 'state\leases'
$registryRoot = Join-Path $runRoot 'state\identities'
$holder = $null
$mappingCreated = $false
$readOnlySource = $null
$preflightLocalPath = $null
$preflightCreated = $false
$checks = [System.Collections.Generic.List[object]]::new()

function Add-Check {
    param([string] $Name)
    $checks.Add([ordered]@{ name = $Name; passed = $true })
}

function Invoke-IdentityGate {
    param(
        [Parameter(Mandatory = $true)] [string] $Command,
        [Parameter(Mandatory = $true)] [string] $Project,
        [string[]] $AdditionalArguments = @()
    )
    $arguments = @(
        $Command,
        '--project', $Project,
        '--lease-root', $leaseRoot,
        '--registry-root', $registryRoot
    ) + $AdditionalArguments
    $output = @(& $gateExecutable @arguments)
    if ($LASTEXITCODE -ne 0 -or $output.Count -lt 1) {
        throw "Identidade gate command '$Command' failed."
    }
    return $output[-1] | ConvertFrom-Json
}

function Start-IdentityHolder {
    param(
        [Parameter(Mandatory = $true)] [string] $Project,
        [Parameter(Mandatory = $true)] [uint32] $PendingDpi
    )
    $start = [System.Diagnostics.ProcessStartInfo]::new()
    $start.FileName = $gateExecutable
    $start.UseShellExecute = $false
    $start.RedirectStandardInput = $true
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    $start.Arguments = (@(
        'hold',
        '--project', $Project,
        '--lease-root', $leaseRoot,
        '--registry-root', $registryRoot,
        '--pending-dpi', $PendingDpi.ToString([Globalization.CultureInfo]::InvariantCulture)
    ) | ForEach-Object { ConvertTo-WindowsProcessArgument $_ }) -join ' '
    $process = [System.Diagnostics.Process]::new()
    $process.StartInfo = $start
    if (-not $process.Start()) {
        throw 'The real holder process did not start.'
    }
    $readyLine = $process.StandardOutput.ReadLine()
    if ([string]::IsNullOrWhiteSpace($readyLine)) {
        $failure = $process.StandardError.ReadToEnd()
        throw "The real holder process exited before readiness: $failure"
    }
    $ready = $readyLine | ConvertFrom-Json
    if ($ready.status -ne 'holding' -or $ready.pid -ne $process.Id) {
        throw 'The holder readiness was not correlated to its real process.'
    }
    return [pscustomobject]@{
        process = $process
        creationTime = [uint64] $process.StartTime.ToFileTimeUtc()
        ready = $ready
    }
}

function Stop-IdentityHolder {
    param([Parameter(Mandatory = $true)] [psobject] $Holder)
    if ($Holder.process.HasExited) {
        throw 'The holder exited before its explicit release.'
    }
    $Holder.process.StandardInput.WriteLine('release')
    $Holder.process.StandardInput.Flush()
    $Holder.process.StandardInput.Close()
    if (-not $Holder.process.WaitForExit(5000)) {
        $Holder.process.Kill()
        $Holder.process.WaitForExit()
        throw 'The holder did not exit after its explicit release.'
    }
    if ($Holder.process.ExitCode -ne 0) {
        throw "The holder exited with code $($Holder.process.ExitCode)."
    }
    $Holder.process.Dispose()
}

function Get-FileSha256 {
    param([Parameter(Mandatory = $true)] [string] $Path)
    return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

try {
    $previousDevDebug = $env:CARGO_PROFILE_DEV_DEBUG
    $previousDevIncremental = $env:CARGO_PROFILE_DEV_INCREMENTAL
    try {
        $env:CARGO_PROFILE_DEV_DEBUG = '0'
        $env:CARGO_PROFILE_DEV_INCREMENTAL = 'false'
        Push-Location $script:WorkspaceRoot
        try {
            & $script:CargoExecutable build -p myalbuns-core --example issue10_identity_gate
            if ($LASTEXITCODE -ne 0) {
                throw 'The public ProjectCore evidence executable did not build.'
            }
        }
        finally {
            Pop-Location
        }
    }
    finally {
        $env:CARGO_PROFILE_DEV_DEBUG = $previousDevDebug
        $env:CARGO_PROFILE_DEV_INCREMENTAL = $previousDevIncremental
    }
    if (-not (Test-Path -LiteralPath $gateExecutable -PathType Leaf)) {
        throw 'The public ProjectCore evidence executable is missing.'
    }
    Add-Check 'public-project-core-gate-build'

    Push-Location $script:WorkspaceRoot
    try {
        & $script:CargoExecutable test `
            -p myalbuns-paths `
            'resolve::windows_identity_tests::physical_identity_comparison_is_closed_across_file_id_domains' `
            -- `
            --exact
        if ($LASTEXITCODE -ne 0) {
            throw 'The mixed Windows file-ID comparison contract failed.'
        }
    }
    finally {
        Pop-Location
    }
    Add-Check 'mixed-file-id-domains-fail-closed'

    $physicalIdentityContracts = @(
        [ordered]@{
            name = 'zero-extended-file-id-sentinel-fails-closed'
            package = 'myalbuns-paths'
            arguments = @(
                '--lib',
                'resolve::windows_identity_tests::an_all_zero_extended_file_id_is_never_authoritative'
            )
        },
        [ordered]@{
            name = 'ones-extended-file-id-sentinel-fails-closed'
            package = 'myalbuns-paths'
            arguments = @(
                '--lib',
                'resolve::windows_identity_tests::an_all_ones_extended_file_id_is_never_authoritative'
            )
        },
        [ordered]@{
            name = 'legacy-file-id-provenance-fails-closed'
            package = 'myalbuns-paths'
            arguments = @(
                '--lib',
                'resolve::windows_identity_tests::a_provenance_free_legacy_file_id_cannot_authorize_same'
            )
        },
        [ordered]@{
            name = 'refs-and-unknown-legacy-ids-fail-closed'
            package = 'myalbuns-paths'
            arguments = @(
                '--lib',
                'resolve::windows_identity_tests::equal_legacy_ids_on_refs_or_an_unknown_filesystem_are_indeterminate'
            )
        },
        [ordered]@{
            name = 'unexpected-file-id-query-error-has-no-fallback'
            package = 'myalbuns-paths'
            arguments = @(
                '--lib',
                'resolve::windows_identity_tests::an_unexpected_extended_file_id_error_never_falls_back_to_legacy_identity'
            )
        },
        [ordered]@{
            name = 'documented-ntfs-legacy-fallback-remains-authoritative'
            package = 'myalbuns-paths'
            arguments = @(
                '--lib',
                'resolve::windows_identity_tests::unsupported_extended_queries_can_use_a_guaranteed_ntfs_legacy_id'
            )
        },
        [ordered]@{
            name = 'native-volume-serial-widths-fail-closed-across-file-id-domains'
            package = 'myalbuns-paths'
            arguments = @(
                '--lib',
                'resolve::windows_identity_tests::mixed_file_id_domains_normalize_native_volume_serial_widths_before_different'
            )
        },
        [ordered]@{
            name = 'real-ntfs-handle-volume-serial-widths-fail-closed'
            package = 'myalbuns-paths'
            arguments = @(
                '--lib',
                'resolve::windows_identity_tests::a_real_ntfs_handle_keeps_mixed_volume_serial_widths_indeterminate'
            )
        },
        [ordered]@{
            name = 'incompatible-lease-identity-never-focuses'
            package = 'myalbuns-core'
            arguments = @(
                '--lib',
                'project_store::identity_lease::tests::active_lease_focus_requires_compatible_authoritative_physical_identity'
            )
        }
    )
    foreach ($contract in $physicalIdentityContracts) {
        Push-Location $script:WorkspaceRoot
        try {
            & $script:CargoExecutable test `
                -p $contract.package `
                @($contract.arguments) `
                -- `
                --exact
            if ($LASTEXITCODE -ne 0) {
                throw "The physical Identidade contract failed: $($contract.name)"
            }
        }
        finally {
            Pop-Location
        }
        Add-Check $contract.name
    }

    $causalTests = @(
        [ordered]@{
            name = 'a-to-b-success-race-fails-closed'
            test = 'a_successful_open_never_authorizes_a_path_replacement_with_the_same_identity'
        },
        [ordered]@{
            name = 'write-protected-volume-offers-save-copy-as'
            test = 'an_external_copy_on_write_protected_media_offers_save_copy_as'
        }
    )
    foreach ($causalTest in $causalTests) {
        Push-Location $script:WorkspaceRoot
        try {
            & $script:CargoExecutable test `
                -p myalbuns-core `
                --test project_identity_transitions `
                $causalTest.test `
                -- `
                --exact
            if ($LASTEXITCODE -ne 0) {
                throw "The causal public ProjectCore test failed: $($causalTest.test)"
            }
        }
        finally {
            Pop-Location
        }
        Add-Check $causalTest.name
    }

    $preflightName = '.myalbuns-issue10-preflight-{0}.tmp' -f `
        [System.Guid]::NewGuid().ToString('N')
    $preflightLocalPath = Join-Path $runRoot $preflightName
    $preflightUncPath = Join-Path $UncRoot $preflightName
    $preflightBytes = [byte[]](1, 2, 3)
    $preflightStream = [System.IO.FileStream]::new(
        $preflightLocalPath,
        [System.IO.FileMode]::CreateNew,
        [System.IO.FileAccess]::Write,
        [System.IO.FileShare]::None
    )
    $preflightCreated = $true
    try {
        $preflightStream.Write($preflightBytes, 0, $preflightBytes.Length)
        $preflightStream.Flush($true)
    }
    finally {
        $preflightStream.Dispose()
    }
    $observedPreflight = [System.IO.File]::ReadAllBytes($preflightUncPath)
    if ([Convert]::ToBase64String($observedPreflight) -ne `
            [Convert]::ToBase64String($preflightBytes)) {
        throw 'The supplied UNC root does not resolve to the isolated gate scratch root.'
    }
    $mappingOutput = @(& net.exe use $mappedDrive $UncRoot /persistent:no 2>&1)
    if ($LASTEXITCODE -ne 0) {
        throw "The real mapped drive could not be created: $($mappingOutput -join ' ')"
    }
    $mappingCreated = $true
    Add-Check 'real-loopback-unc-and-mapped-drive'

    $originalPath = Join-Path $runRoot 'Original.myalbuns'
    $mappedOriginalPath = "$mappedDrive\Original.myalbuns"
    $uncOriginalPath = Join-Path $UncRoot 'Original.myalbuns'
    $created = Invoke-IdentityGate -Command 'create' -Project $originalPath
    if ($created.status -ne 'opened') {
        throw 'The original Project was not created.'
    }
    $holder = Start-IdentityHolder -Project $originalPath -PendingDpi 240
    if (-not $holder.ready.dirty -or $holder.ready.dpi -ne 240) {
        throw 'The real owner did not hold the pending creative revision.'
    }

    $mappedAlias = Invoke-IdentityGate -Command 'open' -Project $mappedOriginalPath
    $uncAlias = Invoke-IdentityGate -Command 'open' -Project $uncOriginalPath
    foreach ($alias in @($mappedAlias, $uncAlias)) {
        if ($alias.status -ne 'focusExisting' `
                -or $alias.projectId -ne $created.projectId `
                -or $alias.ownerProcess.processId -ne $holder.process.Id `
                -or $alias.ownerProcess.creationTime -ne $holder.creationTime) {
            throw 'A physical Windows alias did not focus the existing owner.'
        }
    }
    Add-Check 'mapped-and-unc-aliases-focus-real-owner-process'

    $writableCopyPath = Join-Path $runRoot 'Copia gravavel.myalbuns'
    Copy-Item -LiteralPath $originalPath -Destination $writableCopyPath
    $writableCopy = Invoke-IdentityGate -Command 'open' -Project $writableCopyPath
    if ($writableCopy.status -ne 'opened' `
            -or $writableCopy.projectId -eq $created.projectId `
            -or $writableCopy.namespace -eq $created.namespace `
            -or $writableCopy.dpi -ne 300 `
            -or $writableCopy.dirty `
            -or $writableCopy.canUndo) {
        throw 'The writable external copy was not promoted from persisted source state.'
    }
    $promotedCopyDocument = Get-Content -LiteralPath $writableCopyPath -Raw | ConvertFrom-Json
    if ($promotedCopyDocument.projectId -ne $writableCopy.projectId `
            -or $promotedCopyDocument.revision -ne $created.revision) {
        throw 'The writable copy Identidade was not persisted before the Sessão result.'
    }
    Add-Check 'writable-copy-persists-new-identity-before-session'
    Add-Check 'pending-creative-change-not-in-technical-identity-write'

    $editedCopy = Invoke-IdentityGate `
        -Command 'edit-save' `
        -Project $writableCopyPath `
        -AdditionalArguments @('--dpi', '600')
    $originalDocument = Get-Content -LiteralPath $originalPath -Raw | ConvertFrom-Json
    $copyDocument = Get-Content -LiteralPath $writableCopyPath -Raw | ConvertFrom-Json
    if ($editedCopy.status -ne 'opened' `
            -or $copyDocument.project.document.dpi -ne 600 `
            -or $originalDocument.project.document.dpi -ne 300) {
        throw 'Editing the promoted copy was not isolated from the original.'
    }
    Add-Check 'simultaneous-original-and-copy-editing-isolated'

    Stop-IdentityHolder -Holder $holder
    $holder = $null
    $renamedPath = Join-Path $runRoot 'Renomeado.myalbuns'
    Move-Item -LiteralPath $originalPath -Destination $renamedPath
    $renamed = Invoke-IdentityGate -Command 'open' -Project $renamedPath
    $movedDirectory = Join-Path $runRoot 'Pasta movida'
    New-Item -ItemType Directory -Path $movedDirectory | Out-Null
    $movedPath = Join-Path $movedDirectory 'Movido.myalbuns'
    Move-Item -LiteralPath $renamedPath -Destination $movedPath
    $moved = Invoke-IdentityGate -Command 'open' -Project $movedPath
    if ($renamed.status -ne 'opened' `
            -or $moved.status -ne 'opened' `
            -or $renamed.projectId -ne $created.projectId `
            -or $moved.projectId -ne $created.projectId `
            -or $moved.namespace -ne $created.namespace) {
        throw 'Rename or movement duplicated the persisted Identidade namespace.'
    }
    Add-Check 'rename-and-movement-preserve-identity-and-namespace'

    $legacyOriginalPath = Join-Path $runRoot 'Legado original.myalbuns'
    $legacyFixture = Join-Path `
        $script:WorkspaceRoot `
        'crates\myalbuns-core\tests\fixtures\project_document_v1_migration_input.myalbuns'
    Copy-Item -LiteralPath $legacyFixture -Destination $legacyOriginalPath
    $legacyOriginal = Invoke-IdentityGate -Command 'open' -Project $legacyOriginalPath
    if ($legacyOriginal.status -ne 'opened') {
        throw 'The valid schema v1 source did not establish durable Identidade evidence.'
    }
    $readOnlySource = Join-Path $runRoot 'Copia somente leitura.myalbuns'
    Copy-Item -LiteralPath $legacyOriginalPath -Destination $readOnlySource
    [System.IO.File]::SetAttributes($readOnlySource, [System.IO.FileAttributes]::ReadOnly)
    $readOnlyHash = Get-FileSha256 -Path $readOnlySource
    $readOnlyOpen = Invoke-IdentityGate -Command 'open' -Project $readOnlySource
    if ($readOnlyOpen.status -ne 'externalCopyNotWritable') {
        throw 'The read-only external copy did not offer Salvar cópia como...'
    }
    Add-Check 'read-only-copy-blocked-with-save-copy-as-offer'

    $savedCopyPath = Join-Path $runRoot 'Copia salva.myalbuns'
    $savedCopy = Invoke-IdentityGate `
        -Command 'save-copy-as' `
        -Project $readOnlySource `
        -AdditionalArguments @('--destination', $savedCopyPath)
    $savedDocument = Get-Content -LiteralPath $savedCopyPath -Raw | ConvertFrom-Json
    if ($savedCopy.status -ne 'opened' `
            -or $savedCopy.projectId -eq $legacyOriginal.projectId `
            -or $savedCopy.revision -ne $legacyOriginal.revision `
            -or $savedCopy.savedRevision -ne $legacyOriginal.revision `
            -or $savedCopy.namespace -eq $legacyOriginal.namespace `
            -or $savedCopy.dirty `
            -or $savedCopy.canUndo `
            -or $savedDocument.schemaVersion -ne 3 `
            -or $savedDocument.projectId -ne $savedCopy.projectId `
            -or (Get-FileSha256 -Path $readOnlySource) -ne $readOnlyHash) {
        throw 'Salvar cópia como... did not preserve source, Revision, current schema and isolation.'
    }
    Add-Check 'save-copy-as-preserves-source-revision-and-publishes-current-schema'

    $occupiedPath = Join-Path $runRoot 'Destino ocupado.myalbuns'
    [System.IO.File]::WriteAllText($occupiedPath, 'occupied')
    $occupiedHash = Get-FileSha256 -Path $occupiedPath
    $failedCopy = Invoke-IdentityGate `
        -Command 'save-copy-as' `
        -Project $readOnlySource `
        -AdditionalArguments @('--destination', $occupiedPath)
    if ($failedCopy.status -ne 'destinationConflict' `
            -or (Get-FileSha256 -Path $occupiedPath) -ne $occupiedHash `
            -or (Get-FileSha256 -Path $readOnlySource) -ne $readOnlyHash) {
        throw 'A failed Salvar cópia como... operation changed its source or occupied destination.'
    }
    Add-Check 'failed-save-copy-as-creates-no-editable-session'

    $networkOriginPath = "$mappedDrive\Origem de rede.myalbuns"
    $networkOriginLocalPath = Join-Path $runRoot 'Origem de rede.myalbuns'
    $networkOrigin = Invoke-IdentityGate -Command 'create' -Project $networkOriginPath
    if ($networkOrigin.status -ne 'opened') {
        throw 'The mapped origin was not created.'
    }
    $unavailableCandidatePath = Join-Path $runRoot 'Candidato com origem indisponivel.myalbuns'
    Copy-Item -LiteralPath $networkOriginLocalPath -Destination $unavailableCandidatePath
    $unavailableCandidateHash = Get-FileSha256 -Path $unavailableCandidatePath
    $unmapOutput = @(& net.exe use $mappedDrive /delete /y 2>&1)
    if ($LASTEXITCODE -ne 0) {
        throw "The mapped drive could not be removed: $($unmapOutput -join ' ')"
    }
    $mappingCreated = $false
    $unavailable = Invoke-IdentityGate -Command 'open' -Project $unavailableCandidatePath
    if ($unavailable.status -ne 'identityIndeterminate' `
            -or (Get-FileSha256 -Path $unavailableCandidatePath) -ne $unavailableCandidateHash) {
        throw 'An unavailable previous root did not fail closed without rewriting the candidate.'
    }
    Add-Check 'unavailable-previous-root-fails-closed'

    $sourceSnapshotAfter = Get-GateSourceSnapshot `
        -WorkspaceRoot $script:WorkspaceRoot `
        -EvidencePath $OutputPath
    $report = [ordered]@{
        schemaVersion = 1
        collectedAtUtc = [DateTime]::UtcNow.ToString('o')
        gitCommit = $sourceSnapshotBefore.gitCommit
        sourceInputsDirty = Test-GateSourceSnapshotsDirty `
            -Before $sourceSnapshotBefore `
            -After $sourceSnapshotAfter
        platform = [ordered]@{
            operatingSystem = [System.Environment]::OSVersion.VersionString
            architecture =
                [Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
            uncProvider = 'loopback-administrative-share'
        }
        checks = @($checks)
        evidence = [ordered]@{
            originalProjectId = $created.projectId
            promotedCopyProjectId = $writableCopy.projectId
            saveCopyAsProjectId = $savedCopy.projectId
            ownerProcess = [ordered]@{
                processId = $mappedAlias.ownerProcess.processId
                creationTime = $mappedAlias.ownerProcess.creationTime
            }
            mappedAliasOutcome = $mappedAlias.status
            uncAliasOutcome = $uncAlias.status
            movedNamespaceReused = $moved.namespace -eq $created.namespace
            writableCopyNamespaceIsolated = $writableCopy.namespace -ne $created.namespace
            persistedDpiExcludedPending240 = $promotedCopyDocument.project.document.dpi
            isolatedCopyDpi = $copyDocument.project.document.dpi
            isolatedOriginalDpi = $originalDocument.project.document.dpi
            readOnlySourceSha256 = $readOnlyHash
            readOnlySourcePreserved = (Get-FileSha256 -Path $readOnlySource) -eq $readOnlyHash
            saveCopyAsSourceSchema = 1
            saveCopyAsDestinationSchema = $savedDocument.schemaVersion
            saveCopyAsRevision = $savedCopy.revision
            unavailableOriginOutcome = $unavailable.status
            unavailableCandidatePreserved =
                (Get-FileSha256 -Path $unavailableCandidatePath) -eq $unavailableCandidateHash
        }
    }
    $json = $report | ConvertTo-Json -Depth 6
    $outputDirectory = Split-Path -Parent $OutputPath
    New-Item -ItemType Directory -Force -Path $outputDirectory | Out-Null
    [System.IO.File]::WriteAllText(
        $OutputPath,
        $json + [System.Environment]::NewLine,
        [System.Text.UTF8Encoding]::new($false)
    )
    Write-Output "Issue 10 Identidade gate report: $OutputPath"
    Write-Output $json
}
finally {
    if ($null -ne $holder) {
        if (-not $holder.process.HasExited) {
            $holder.process.Kill()
            $holder.process.WaitForExit()
        }
        $holder.process.Dispose()
    }
    if ($null -ne $readOnlySource -and (Test-Path -LiteralPath $readOnlySource -PathType Leaf)) {
        [System.IO.File]::SetAttributes($readOnlySource, [System.IO.FileAttributes]::Normal)
    }
    if ($mappingCreated) {
        & net.exe use $mappedDrive /delete /y | Out-Null
    }
    if ($preflightCreated `
            -and $null -ne $preflightLocalPath `
            -and [System.IO.File]::Exists($preflightLocalPath)) {
        [System.IO.File]::Delete($preflightLocalPath)
        $preflightCreated = $false
    }
    Remove-GateScratchDirectory -Path $runRoot -AllowedParent $scratchRoot
}
