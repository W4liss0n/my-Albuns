param(
    [string] $OutputPath,
    [ValidateSet('normal', 'batch', 'project_open')]
    [string] $Suite = 'normal',
    [ValidateRange(10, 300)]
    [int] $ProbeTimeoutSeconds = 90,
    [ValidateRange(1, 8)]
    [int] $CargoBuildJobs = 1
)

$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'Local-Toolchain.ps1')
Initialize-MyAlbunsToolchain
. (Join-Path $PSScriptRoot 'Evidence-BuildInputs.ps1')
. (Join-Path $PSScriptRoot 'Test-ProjectOpenGate.ps1')

if ($env:OS -ne 'Windows_NT') {
    throw 'The OperationGate gate must run on Windows.'
}

$runnerMutex = [System.Threading.Mutex]::new(
    $false,
    'Local\MyAlbuns.OperationGateEvidence.v1'
)
$runnerMutexHeld = $false
try {
    $runnerMutexHeld = $runnerMutex.WaitOne(0)
}
catch [System.Threading.AbandonedMutexException] {
    $runnerMutexHeld = $true
}
if (-not $runnerMutexHeld) {
    $runnerMutex.Dispose()
    throw 'Another OperationGate evidence runner is already using the shared build artifacts.'
}

if ([string]::IsNullOrWhiteSpace($OutputPath)) {
    $defaultArtifact = switch ($Suite) {
        'batch' {
            'docs\research\artifacts\0011-batch-operation-lease.json'
        }
        'project_open' {
            'docs\research\artifacts\0012-operation-gate-project-lock.json'
        }
        default {
            'docs\research\artifacts\0010-export-terminal-matrix.json'
        }
    }
    $OutputPath = Join-Path `
        $script:WorkspaceRoot `
        $defaultArtifact
}
elseif (-not [System.IO.Path]::IsPathRooted($OutputPath)) {
    $OutputPath = Join-Path $script:WorkspaceRoot $OutputPath
}
$OutputPath = [System.IO.Path]::GetFullPath($OutputPath)

$probeParent = [System.IO.Path]::GetFullPath(
    (Join-Path $script:WorkspaceRoot '.scratch\operation-gate-probe')
)
$runId = "$PID-$([DateTime]::UtcNow.Ticks)"
$independentRoot = [System.IO.Path]::GetFullPath(
    (Join-Path $probeParent "run-$runId-independent")
)
$multiwindowRoot = [System.IO.Path]::GetFullPath(
    (Join-Path $probeParent "run-$runId-multiwindow")
)
$terminalScenarios = @('success', 'failure', 'cancellation', 'owner_death')
$terminalRoots = [ordered]@{
    independent = [ordered]@{}
    multiwindow = [ordered]@{}
}
foreach ($topology in @('independent', 'multiwindow')) {
    foreach ($scenario in $terminalScenarios) {
        $terminalRoots[$topology][$scenario] = [System.IO.Path]::GetFullPath(
            (Join-Path $probeParent "run-$runId-$topology-$scenario")
        )
    }
}
$batchScenarios = @(
    'success',
    'before_preparation',
    'between_promotions',
    'owner_death'
)
$batchRoots = [ordered]@{}
foreach ($scenario in $batchScenarios) {
    $batchRoots[$scenario] = [System.IO.Path]::GetFullPath(
        (Join-Path $probeParent "run-$runId-batch-$scenario")
    )
}
$projectOpenContext = if ($Suite -eq 'project_open') {
    New-ProjectOpenGateContext -ProbeParent $probeParent -RunId $runId
}
$fixtureRoots = switch ($Suite) {
    'normal' {
        @($independentRoot, $multiwindowRoot) + @(
            foreach ($topology in @('independent', 'multiwindow')) {
                foreach ($scenario in $terminalScenarios) {
                    $terminalRoots[$topology][$scenario]
                }
            }
        )
    }
    'batch' {
        @($batchScenarios | ForEach-Object { $batchRoots[$_] })
    }
    'project_open' {
        @($projectOpenContext.FixtureRoots)
    }
}
foreach ($fixtureRoot in $fixtureRoots) {
    if (-not $fixtureRoot.StartsWith(
            $probeParent + [System.IO.Path]::DirectorySeparatorChar,
            [System.StringComparison]::OrdinalIgnoreCase
        )) {
        throw 'An OperationGate probe root escaped the workspace scratch directory.'
    }
    New-Item -ItemType Directory -Force -Path $fixtureRoot | Out-Null
}

$targetDirectory = [System.IO.Path]::GetFullPath(
    (Join-Path $script:WorkspaceRoot 'target\operation-gate')
)
$rustCheckTargetDirectory = [System.IO.Path]::GetFullPath(
    (Join-Path $script:WorkspaceRoot 'target')
)
$executablePath = Join-Path $targetDirectory 'release\myalbuns-desktop.exe'
$executableRelativePath = 'target/operation-gate/release/myalbuns-desktop.exe'
$imagingExecutablePath = Join-Path $targetDirectory 'release\myalbuns-imaging.exe'
$imagingExecutableRelativePath =
    'target/operation-gate/release/myalbuns-imaging.exe'
$startedProcesses =
    [System.Collections.Generic.List[System.Diagnostics.Process]]::new()
$checks = [System.Collections.Generic.List[object]]::new()
$previousCargoTarget = [System.Environment]::GetEnvironmentVariable(
    'CARGO_TARGET_DIR',
    [System.EnvironmentVariableTarget]::Process
)
$previousCargoBuildJobs = [System.Environment]::GetEnvironmentVariable(
    'CARGO_BUILD_JOBS',
    [System.EnvironmentVariableTarget]::Process
)
$locationWasPushed = $false
$report = $null

function Set-OperationProbeEnvironmentValue {
    param(
        [Parameter(Mandatory = $true)]
        [string] $Name,
        [AllowNull()]
        [string] $Value
    )

    [System.Environment]::SetEnvironmentVariable(
        $Name,
        $Value,
        [System.EnvironmentVariableTarget]::Process
    )
}

function Assert-BatchScenarioName {
    param([Parameter(Mandatory = $true)][string] $Scenario)

    if ($script:batchScenarios -cnotcontains $Scenario) {
        throw "Unknown Batch lease scenario '$Scenario'."
    }
}


function Invoke-RustCheck {
    param(
        [Parameter(Mandatory = $true)]
        [string] $Name,
        [Parameter(Mandatory = $true)]
        [string[]] $Arguments
    )

    $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    $previousErrorActionPreference = $ErrorActionPreference
    try {
        $ErrorActionPreference = 'Continue'
        $commandOutput = @(& $script:CargoExecutable @Arguments 2>&1)
        $exitCode = $LASTEXITCODE
    }
    finally {
        $ErrorActionPreference = $previousErrorActionPreference
    }
    $stopwatch.Stop()
    foreach ($line in $commandOutput) {
        Write-Host $line
    }
    if ($exitCode -ne 0) {
        throw "OperationGate check '$Name' failed with exit code $exitCode."
    }
    $transcript = $commandOutput -join [System.Environment]::NewLine
    if ($transcript -notmatch '(?m)^running [1-9][0-9]* tests?') {
        throw "OperationGate check '$Name' did not execute any test."
    }
    if (
        $transcript -notmatch
            '(?m)^test result: ok\. [1-9][0-9]* passed; 0 failed;'
    ) {
        throw "OperationGate check '$Name' did not pass any non-ignored test."
    }
    $checks.Add([ordered]@{
        name = $Name
        passed = $true
        elapsedMs = [long] $stopwatch.ElapsedMilliseconds
    })
}

function Start-OperationProbeProcess {
    param(
        [Parameter(Mandatory = $true)]
        [ValidateSet('independent', 'multiwindow')]
        [string] $Topology,
        [Parameter(Mandatory = $true)]
        [string] $ProbeRoot,
        [ValidateSet('a', 'b')]
        [string] $ProjectSlot,
        [ValidateSet('success', 'failure', 'cancellation', 'owner_death')]
        [string] $TerminalScenario,
        [ValidateSet('matrix', 'successor')]
        [string] $TerminalPhase = 'matrix',
        [string] $BatchScenario,
        [string] $ProjectOpenScenario
    )

    $hasTerminalScenario = -not [string]::IsNullOrWhiteSpace(
        $TerminalScenario
    )
    $hasBatchScenario = -not [string]::IsNullOrWhiteSpace($BatchScenario)
    $hasProjectOpenScenario = -not [string]::IsNullOrWhiteSpace(
        $ProjectOpenScenario
    )
    $explicitScenarioCount = @(
        $hasTerminalScenario,
        $hasBatchScenario,
        $hasProjectOpenScenario
    ).Where({ $_ }).Count
    if ($explicitScenarioCount -gt 1) {
        throw 'A probe process cannot run terminal, Batch, and Project opening scenarios together.'
    }
    if ($hasBatchScenario) {
        Assert-BatchScenarioName -Scenario $BatchScenario
        if ($Topology -cne 'independent') {
            throw 'The Batch lease probe supports only independent hosts.'
        }
    }
    $environment = [ordered]@{
        MYALBUNS_PROCESS_ROLE = $null
        MYALBUNS_TOPOLOGY_SPIKE = $Topology
        MYALBUNS_TOPOLOGY_PROJECT = $ProjectSlot
        MYALBUNS_TOPOLOGY_RUN_ID = $null
        MYALBUNS_TOPOLOGY_CORPUS_MANIFEST = $null
        MYALBUNS_TOPOLOGY_PROBE_GATE = $null
        MYALBUNS_TOPOLOGY_EXPORT_GATE = $null
        MYALBUNS_TOPOLOGY_FAULT_GATE = $null
        MYALBUNS_TOPOLOGY_FAULT_OUTPUT_ROOT = $null
        MYALBUNS_TOPOLOGY_PROJECT_A_SOURCE = $null
        MYALBUNS_TOPOLOGY_PROJECT_B_SOURCE = $null
        MYALBUNS_GLOBAL_SPIKE_ENDPOINT = $null
        MYALBUNS_OPERATION_GATE_PROBE_ROOT = $null
        MYALBUNS_EXPORT_TERMINAL_PROBE_ROOT = $null
        MYALBUNS_EXPORT_TERMINAL_PROBE_SCENARIO = $null
        MYALBUNS_EXPORT_TERMINAL_PROBE_PHASE = $null
        MYALBUNS_BATCH_LEASE_PROBE_ROOT = $null
        MYALBUNS_BATCH_LEASE_PROBE_SCENARIO = $null
    }
    foreach ($name in Get-ProjectOpenProbeEnvironmentNames) {
        $environment[$name] = $null
    }
    if ($hasProjectOpenScenario) {
        $projectOpenEnvironment = Get-ProjectOpenProbeEnvironment `
            -ProbeRoot $ProbeRoot `
            -Scenario $ProjectOpenScenario `
            -Topology $Topology
        foreach ($entry in $projectOpenEnvironment.GetEnumerator()) {
            $environment[$entry.Key] = $entry.Value
        }
    }
    elseif ($hasBatchScenario) {
        $environment.MYALBUNS_BATCH_LEASE_PROBE_ROOT = $ProbeRoot
        $environment.MYALBUNS_BATCH_LEASE_PROBE_SCENARIO = $BatchScenario
    }
    elseif (-not $hasTerminalScenario) {
        $environment.MYALBUNS_OPERATION_GATE_PROBE_ROOT = $ProbeRoot
    }
    else {
        $environment.MYALBUNS_EXPORT_TERMINAL_PROBE_ROOT = $ProbeRoot
        $environment.MYALBUNS_EXPORT_TERMINAL_PROBE_SCENARIO =
            $TerminalScenario
        $environment.MYALBUNS_EXPORT_TERMINAL_PROBE_PHASE = $TerminalPhase
    }
    if ($Topology -eq 'multiwindow') {
        $environment.MYALBUNS_TOPOLOGY_PROJECT = $null
    }

    $previous = @{}
    try {
        foreach ($entry in $environment.GetEnumerator()) {
            $previous[$entry.Key] =
                [System.Environment]::GetEnvironmentVariable(
                    $entry.Key,
                    [System.EnvironmentVariableTarget]::Process
                )
            Set-OperationProbeEnvironmentValue `
                -Name $entry.Key `
                -Value $entry.Value
        }

        $process = Start-Process `
            -FilePath $executablePath `
            -WorkingDirectory $script:WorkspaceRoot `
            -PassThru `
            -WindowStyle Hidden
        $startedProcesses.Add($process)
        return $process
    }
    finally {
        foreach ($entry in $previous.GetEnumerator()) {
            Set-OperationProbeEnvironmentValue `
                -Name $entry.Key `
                -Value $entry.Value
        }
    }
}

function Assert-OwnedOperationProbeProcess {
    param(
        [Parameter(Mandatory = $true)]
        [System.Diagnostics.Process] $Process
    )

    if ($Process.HasExited) {
        return $false
    }
    $processPath = $Process.MainModule.FileName
    if ([string]::IsNullOrWhiteSpace($processPath) -or -not [string]::Equals(
            [System.IO.Path]::GetFullPath($processPath),
            [System.IO.Path]::GetFullPath($executablePath),
            [System.StringComparison]::OrdinalIgnoreCase
        )) {
        throw (
            "Process $($Process.Id) does not belong to the " +
            'OperationGate probe executable.'
        )
    }
    return $true
}

function Stop-OwnedOperationProbeProcess {
    param(
        [Parameter(Mandatory = $true)]
        [System.Diagnostics.Process] $Process,
        [switch] $RequireLiveKill
    )

    $confirmedStopped = $false
    try {
        if ($Process.HasExited) {
            if ($RequireLiveKill) {
                throw (
                    "OperationGate probe process $($Process.Id) exited " +
                    'before the controlled owner-death injection.'
                )
            }
            $confirmedStopped = $true
            return
        }
        if (Assert-OwnedOperationProbeProcess -Process $Process) {
            $Process.Kill()
            if (-not $Process.WaitForExit(10000)) {
                throw (
                    "OperationGate probe process $($Process.Id) " +
                    'did not terminate.'
                )
            }
            $confirmedStopped = $true
        }
    }
    finally {
        if (-not $confirmedStopped) {
            try {
                $confirmedStopped = $Process.HasExited
            }
            catch {
                $confirmedStopped = $false
            }
        }
        if ($confirmedStopped) {
            [void] $startedProcesses.Remove($Process)
            $Process.Dispose()
        }
    }
}

function Get-ProbeFailureDiagnostic {
    param([Parameter(Mandatory = $true)][string] $ProbeRoot)

    $failures = @(
        Get-ChildItem `
            -LiteralPath $ProbeRoot `
            -Filter 'failure-*.json' `
            -File `
            -ErrorAction SilentlyContinue |
            Sort-Object Name |
            ForEach-Object {
                try {
                    Get-Content -LiteralPath $_.FullName -Raw -Encoding utf8
                }
                catch {
                    "Unreadable failure file '$($_.Name)': $($_.Exception.Message)"
                }
            }
    )
    if ($failures.Count -eq 0) {
        return 'No typed probe failure file was produced.'
    }
    return $failures -join ' | '
}

function Assert-NoDuplicateJsonPropertyNames {
    param([Parameter(Mandatory = $true)][string] $Json)

    # ConvertFrom-Json accepts duplicate names, so walk the JSON tokens first.
    # Each object receives its own ordinal set: repeated names in two distinct
    # outputEvidence entries remain valid, while duplicates in either object do not.
    $scopes = [System.Collections.Generic.List[object]]::new()
    for ($index = 0; $index -lt $Json.Length; $index++) {
        $character = $Json[$index]
        if ($character -eq '{') {
            $scopes.Add(
                [System.Collections.Generic.HashSet[string]]::new(
                    [System.StringComparer]::Ordinal
                )
            )
            continue
        }
        if ($character -eq '[') {
            $scopes.Add([System.DBNull]::Value)
            continue
        }
        if ($character -eq '}' -or $character -eq ']') {
            if ($scopes.Count -eq 0) {
                throw 'The OperationGate event contains unbalanced JSON scopes.'
            }
            $scopes.RemoveAt($scopes.Count - 1)
            continue
        }
        if ($character -ne '"') {
            continue
        }

        $tokenStart = $index
        $escaped = $false
        for ($index++; $index -lt $Json.Length; $index++) {
            $tokenCharacter = $Json[$index]
            if ($escaped) {
                $escaped = $false
                continue
            }
            if ($tokenCharacter -eq '\') {
                $escaped = $true
                continue
            }
            if ($tokenCharacter -eq '"') {
                break
            }
        }
        if ($index -ge $Json.Length) {
            throw 'The OperationGate event contains an unterminated JSON string.'
        }
        $lookahead = $index + 1
        while (
            $lookahead -lt $Json.Length -and
            [char]::IsWhiteSpace($Json[$lookahead])
        ) {
            $lookahead++
        }
        if (
            $lookahead -ge $Json.Length -or
            $Json[$lookahead] -ne ':'
        ) {
            continue
        }
        if (
            $scopes.Count -eq 0 -or
            $scopes[$scopes.Count - 1] -isnot
                [System.Collections.Generic.HashSet[string]]
        ) {
            throw 'The OperationGate event contains a property outside an object.'
        }
        $rawName = $Json.Substring($tokenStart, $index - $tokenStart + 1)
        $name = $rawName | ConvertFrom-Json
        $seenNames = $scopes[$scopes.Count - 1]
        if (-not $seenNames.Add([string] $name)) {
            throw "The OperationGate event repeats the JSON field '$name'."
        }
    }
    if ($scopes.Count -ne 0) {
        throw 'The OperationGate event contains unbalanced JSON scopes.'
    }
}

function Read-StrictJsonObject {
    param(
        [Parameter(Mandatory = $true)]
        [string] $Path,
        [Parameter(Mandatory = $true)]
        [string] $SchemaLabel,
        [Parameter(Mandatory = $true)]
        [string[]] $ExpectedNames
    )

    $json = [System.IO.File]::ReadAllText(
        $Path,
        [System.Text.Encoding]::UTF8
    )
    $root = $json | ConvertFrom-Json
    Assert-NoDuplicateJsonPropertyNames -Json $json
    if (
        $null -eq $root -or
        $root -isnot [System.Management.Automation.PSCustomObject]
    ) {
        throw "$SchemaLabel event root must be a JSON object."
    }

    $actualNames = @($root.PSObject.Properties.Name)
    $missingNames = @(
        $ExpectedNames |
            Where-Object { $actualNames -cnotcontains $_ }
    )
    $unexpectedNames = @(
        $actualNames |
            Where-Object { $ExpectedNames -cnotcontains $_ }
    )
    if ($missingNames.Count -gt 0 -or $unexpectedNames.Count -gt 0) {
        throw (
            "$SchemaLabel event fields differ from the closed schema. " +
            "Missing: $($missingNames -join ', '); " +
            "unexpected: $($unexpectedNames -join ', ')."
        )
    }
    return $root
}

function Read-StrictProbeEvent {
    param([Parameter(Mandatory = $true)][string] $Path)

    $expectedNames = @(
        'schemaVersion',
        'processId',
        'topology',
        'windowLabel',
        'state',
        'operationMode'
    )
    $root = Read-StrictJsonObject `
        -Path $Path `
        -SchemaLabel 'The OperationGate' `
        -ExpectedNames $expectedNames

    foreach ($name in @('schemaVersion', 'processId')) {
        $value = $root.$name
        if ($value -isnot [int] -and $value -isnot [long]) {
            throw "OperationGate field '$name' must be an integer."
        }
        if ($value -lt [int]::MinValue -or $value -gt [int]::MaxValue) {
            throw "OperationGate field '$name' must be a 32-bit integer."
        }
    }

    foreach ($name in @(
            'topology',
            'windowLabel',
            'state',
            'operationMode'
        )) {
        if ($root.$name -isnot [string]) {
            throw "OperationGate field '$name' must be a string."
        }
    }

    return [ordered]@{
        schemaVersion = [int] $root.schemaVersion
        processId = [int] $root.processId
        topology = [string] $root.topology
        windowLabel = [string] $root.windowLabel
        state = [string] $root.state
        operationMode = [string] $root.operationMode
    }
}

function Read-StrictExportTerminalEvent {
    param([Parameter(Mandatory = $true)][string] $Path)

    $expectedNames = @(
        'schemaVersion',
        'processId',
        'topology',
        'windowLabel',
        'scenario',
        'state',
        'operationMode',
        'operationId',
        'terminal',
        'progressStages',
        'cancellationDisposition',
        'resources',
        'resourceState',
        'outputBytes'
    )
    $root = Read-StrictJsonObject `
        -Path $Path `
        -SchemaLabel 'The Export terminal' `
        -ExpectedNames $expectedNames

    foreach ($name in @('schemaVersion', 'processId')) {
        $value = $root.$name
        if ($value -isnot [int] -and $value -isnot [long]) {
            throw "Export terminal field '$name' must be an integer."
        }
        if ($value -lt [int]::MinValue -or $value -gt [int]::MaxValue) {
            throw "Export terminal field '$name' must be a 32-bit integer."
        }
    }
    foreach ($name in @(
            'topology',
            'windowLabel',
            'scenario',
            'state',
            'operationMode',
            'resourceState'
        )) {
        if ($root.$name -isnot [string]) {
            throw "Export terminal field '$name' must be a string."
        }
    }
    foreach ($name in @('operationId', 'terminal', 'cancellationDisposition')) {
        if ($null -ne $root.$name -and $root.$name -isnot [string]) {
            throw "Export terminal field '$name' must be a string or null."
        }
    }
    foreach ($name in @('progressStages', 'resources')) {
        if ($root.$name -is [string] -or $null -eq $root.$name) {
            throw "Export terminal field '$name' must be an array."
        }
        foreach ($value in @($root.$name)) {
            if ($value -isnot [string]) {
                throw "Export terminal field '$name' must contain strings."
            }
        }
    }
    if (
        $null -ne $root.outputBytes -and
        $root.outputBytes -isnot [int] -and
        $root.outputBytes -isnot [long]
    ) {
        throw "Export terminal field 'outputBytes' must be an integer or null."
    }
    if ($null -ne $root.outputBytes -and $root.outputBytes -lt 0) {
        throw "Export terminal field 'outputBytes' cannot be negative."
    }

    return [ordered]@{
        schemaVersion = [int] $root.schemaVersion
        processId = [int] $root.processId
        topology = [string] $root.topology
        windowLabel = [string] $root.windowLabel
        scenario = [string] $root.scenario
        state = [string] $root.state
        operationMode = [string] $root.operationMode
        operationId = if ($null -eq $root.operationId) {
            $null
        }
        else {
            [string] $root.operationId
        }
        terminal = if ($null -eq $root.terminal) {
            $null
        }
        else {
            [string] $root.terminal
        }
        progressStages = @($root.progressStages | ForEach-Object { [string] $_ })
        cancellationDisposition = if ($null -eq $root.cancellationDisposition) {
            $null
        }
        else {
            [string] $root.cancellationDisposition
        }
        resources = @($root.resources | ForEach-Object { [string] $_ })
        resourceState = [string] $root.resourceState
        outputBytes = if ($null -eq $root.outputBytes) {
            $null
        }
        else {
            [long] $root.outputBytes
        }
    }
}

function Read-StrictBatchLeaseEvent {
    param([Parameter(Mandatory = $true)][string] $Path)

    $expectedNames = @(
        'schemaVersion',
        'processId',
        'role',
        'scenario',
        'state',
        'operationMode',
        'operationId',
        'terminal',
        'itemIndex',
        'totalItems',
        'completedItems',
        'promotedOutputs',
        'totalOutputs',
        'failureStage',
        'progressStages',
        'resources',
        'resourceState',
        'outputEvidence',
        'outputBytes'
    )
    $root = Read-StrictJsonObject `
        -Path $Path `
        -SchemaLabel 'The Batch lease' `
        -ExpectedNames $expectedNames

    foreach ($name in @(
            'schemaVersion',
            'processId',
            'totalItems',
            'completedItems'
        )) {
        $value = $root.$name
        if ($value -isnot [int] -and $value -isnot [long]) {
            throw "Batch lease field '$name' must be an integer."
        }
        if ($value -lt 0 -or $value -gt [int]::MaxValue) {
            throw "Batch lease field '$name' must be a non-negative 32-bit integer."
        }
    }
    foreach ($name in @(
            'itemIndex',
            'promotedOutputs',
            'totalOutputs',
            'outputBytes'
        )) {
        $value = $root.$name
        if (
            $null -ne $value -and
            $value -isnot [int] -and
            $value -isnot [long]
        ) {
            throw "Batch lease field '$name' must be an integer or null."
        }
        if ($null -ne $value -and $value -lt 0) {
            throw "Batch lease field '$name' cannot be negative."
        }
    }
    foreach ($name in @(
            'role',
            'scenario',
            'state',
            'operationMode',
            'resourceState'
        )) {
        if ($root.$name -isnot [string]) {
            throw "Batch lease field '$name' must be a string."
        }
    }
    foreach ($name in @('operationId', 'terminal', 'failureStage')) {
        if ($null -ne $root.$name -and $root.$name -isnot [string]) {
            throw "Batch lease field '$name' must be a string or null."
        }
    }
    foreach ($name in @('progressStages', 'resources')) {
        if ($root.$name -isnot [System.Array]) {
            throw "Batch lease field '$name' must be an array."
        }
        foreach ($value in @($root.$name)) {
            if ($value -isnot [string]) {
                throw "Batch lease field '$name' must contain strings."
            }
        }
    }
    if ($root.outputEvidence -isnot [System.Array]) {
        throw "Batch lease field 'outputEvidence' must be an array."
    }
    $outputEvidence = @(
        foreach ($entry in @($root.outputEvidence)) {
            if ($entry -isnot [System.Management.Automation.PSCustomObject]) {
                throw "Batch lease field 'outputEvidence' must contain objects."
            }
            $evidenceNames = @($entry.PSObject.Properties.Name)
            $expectedEvidenceNames = @('name', 'bytes', 'sha256')
            if (
                @($expectedEvidenceNames | Where-Object {
                    $evidenceNames -cnotcontains $_
                }).Count -gt 0 -or
                @($evidenceNames | Where-Object {
                    $expectedEvidenceNames -cnotcontains $_
                }).Count -gt 0
            ) {
                throw (
                    "Batch lease outputEvidence fields differ from the " +
                    'closed schema.'
                )
            }
            if (
                $entry.name -isnot [string] -or
                [string]::IsNullOrWhiteSpace($entry.name)
            ) {
                throw "Batch lease outputEvidence field 'name' must be text."
            }
            if (
                ($entry.bytes -isnot [int] -and $entry.bytes -isnot [long]) -or
                $entry.bytes -le 0
            ) {
                throw "Batch lease outputEvidence field 'bytes' must be positive."
            }
            if (
                $entry.sha256 -isnot [string] -or
                $entry.sha256 -cnotmatch '^[0-9a-f]{64}$'
            ) {
                throw "Batch lease outputEvidence field 'sha256' is invalid."
            }
            [ordered]@{
                name = [string] $entry.name
                bytes = [long] $entry.bytes
                sha256 = [string] $entry.sha256
            }
        }
    )

    return [ordered]@{
        schemaVersion = [int] $root.schemaVersion
        processId = [int] $root.processId
        role = [string] $root.role
        scenario = [string] $root.scenario
        state = [string] $root.state
        operationMode = [string] $root.operationMode
        operationId = if ($null -eq $root.operationId) {
            $null
        }
        else {
            [string] $root.operationId
        }
        terminal = if ($null -eq $root.terminal) {
            $null
        }
        else {
            [string] $root.terminal
        }
        itemIndex = if ($null -eq $root.itemIndex) {
            $null
        }
        else {
            [int] $root.itemIndex
        }
        totalItems = [int] $root.totalItems
        completedItems = [int] $root.completedItems
        promotedOutputs = if ($null -eq $root.promotedOutputs) {
            $null
        }
        else {
            [int] $root.promotedOutputs
        }
        totalOutputs = if ($null -eq $root.totalOutputs) {
            $null
        }
        else {
            [int] $root.totalOutputs
        }
        failureStage = if ($null -eq $root.failureStage) {
            $null
        }
        else {
            [string] $root.failureStage
        }
        progressStages = @(
            $root.progressStages | ForEach-Object { [string] $_ }
        )
        resources = @($root.resources | ForEach-Object { [string] $_ })
        resourceState = [string] $root.resourceState
        outputEvidence = @($outputEvidence)
        outputBytes = if ($null -eq $root.outputBytes) {
            $null
        }
        else {
            [long] $root.outputBytes
        }
    }
}


function Test-StrictProbeEventParser {
    param([Parameter(Mandatory = $true)][string] $ProbeRoot)

    $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    $path = Join-Path $ProbeRoot 'parser-contract.json'
    $valid = (
        '{"schemaVersion":1,"processId":42,' +
        '"topology":"independent","windowLabel":"main",' +
        '"state":"owner_ready","operationMode":"normal_export"}'
    )
    [System.IO.File]::WriteAllText(
        $path,
        $valid,
        [System.Text.UTF8Encoding]::new($false)
    )
    $event = Read-StrictProbeEvent -Path $path
    if ($event.schemaVersion -ne 1 -or $event.processId -ne 42) {
        throw 'The OperationGate event parser changed valid numeric fields.'
    }

    $duplicate = (
        '{"schemaVersion":1,"schema\u0056ersion":2,"processId":42,' +
        '"topology":"independent","windowLabel":"main",' +
        '"state":"owner_ready","operationMode":"normal_export"}'
    )
    [System.IO.File]::WriteAllText(
        $path,
        $duplicate,
        [System.Text.UTF8Encoding]::new($false)
    )
    try {
        [void] (Read-StrictProbeEvent -Path $path)
        throw 'The OperationGate event parser accepted a duplicate JSON field.'
    }
    catch {
        if (
            $_.Exception.Message -notmatch
                '^The OperationGate event repeats the JSON field'
        ) {
            throw
        }
    }

    $stopwatch.Stop()
    $checks.Add([ordered]@{
        name = 'strict-probe-json-parser'
        passed = $true
        elapsedMs = [long] $stopwatch.ElapsedMilliseconds
    })
}

function Test-StrictExportTerminalEventParser {
    param([Parameter(Mandatory = $true)][string] $ProbeRoot)

    $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    $path = Join-Path $ProbeRoot 'terminal-parser-contract.json'
    $valid = @'
{"schemaVersion":1,"processId":42,"topology":"multiwindow","windowLabel":"main","scenario":"success","state":"owner_ready","operationMode":"normal_export","operationId":"export-42","terminal":null,"progressStages":["preparing"],"cancellationDisposition":null,"resources":["operation_gate","cache_pause","processor_reservation"],"resourceState":"held","outputBytes":null}
'@
    [System.IO.File]::WriteAllText(
        $path,
        $valid.Trim(),
        [System.Text.UTF8Encoding]::new($false)
    )
    $event = Read-StrictExportTerminalEvent -Path $path
    if (
        $event.schemaVersion -ne 1 -or
        $event.processId -ne 42 -or
        @($event.progressStages).Count -ne 1 -or
        @($event.resources).Count -ne 3
    ) {
        throw 'The Export terminal parser changed a valid event.'
    }

    $stopwatch.Stop()
    $checks.Add([ordered]@{
        name = 'strict-export-terminal-json-parser'
        passed = $true
        elapsedMs = [long] $stopwatch.ElapsedMilliseconds
    })
}

function Test-StrictBatchLeaseEventParser {
    param([Parameter(Mandatory = $true)][string] $ProbeRoot)

    $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    $path = Join-Path $ProbeRoot 'batch-parser-contract.json'
    $valid = @'
{"schemaVersion":1,"processId":42,"role":"owner","scenario":"success","state":"owner_terminal","operationMode":"batch_exclusive","operationId":"batch-42-success","terminal":"success","itemIndex":null,"totalItems":2,"completedItems":2,"promotedOutputs":2,"totalOutputs":2,"failureStage":null,"progressStages":["0:preparing","0:completed","1:preparing","1:completed"],"resources":["operation_gate","cache_pause","processor_reservation"],"resourceState":"released","outputEvidence":[{"name":"one.png","bytes":10,"sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},{"name":"two.png","bytes":20,"sha256":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"}],"outputBytes":null}
'@
    [System.IO.File]::WriteAllText(
        $path,
        $valid.Trim(),
        [System.Text.UTF8Encoding]::new($false)
    )
    $event = Read-StrictBatchLeaseEvent -Path $path
    if (
        $event.schemaVersion -ne 1 -or
        $event.processId -ne 42 -or
        @($event.outputEvidence).Count -ne 2
    ) {
        throw 'The Batch lease parser changed a valid event.'
    }

    $invalidCases = @(
        [ordered]@{
            label = 'duplicate field'
            json = $valid.Replace(
                '"schemaVersion":1',
                '"schemaVersion":1,"schema\u0056ersion":2'
            )
            error = '^The OperationGate event repeats the JSON field'
        },
        [ordered]@{
            label = 'unexpected field'
            json = $valid.Replace(
                '"outputBytes":null}',
                '"outputBytes":null,"unexpected":true}'
            )
            error = '^The Batch lease event fields differ from the closed schema'
        },
        [ordered]@{
            label = 'mis-cased field'
            json = $valid.Replace('"resourceState":', '"ResourceState":')
            error = '^The Batch lease event fields differ from the closed schema'
        },
        [ordered]@{
            label = 'nested duplicate field'
            json = $valid.Replace(
                '"name":"one.png"',
                '"name":"one.png","n\u0061me":"duplicate.png"'
            )
            error = '^The OperationGate event repeats the JSON field'
        }
    )
    foreach ($invalidCase in $invalidCases) {
        [System.IO.File]::WriteAllText(
            $path,
            [string] $invalidCase.json,
            [System.Text.UTF8Encoding]::new($false)
        )
        try {
            [void] (Read-StrictBatchLeaseEvent -Path $path)
            throw (
                "The Batch lease parser accepted $($invalidCase.label)."
            )
        }
        catch {
            if ($_.Exception.Message -cnotmatch $invalidCase.error) {
                throw
            }
        }
    }

    $stopwatch.Stop()
    $checks.Add([ordered]@{
        name = 'strict-batch-lease-json-parser'
        passed = $true
        elapsedMs = [long] $stopwatch.ElapsedMilliseconds
    })
}


function Wait-ForProbeEvent {
    param(
        [Parameter(Mandatory = $true)]
        [string] $Path,
        [Parameter(Mandatory = $true)]
        [System.Diagnostics.Process[]] $Processes,
        [Parameter(Mandatory = $true)]
        [string] $ProbeRoot,
        [ValidateSet(
            'operation',
            'export_terminal',
            'batch_lease',
            'project_open'
        )]
        [string] $Contract = 'operation'
    )

    $deadline = [DateTimeOffset]::UtcNow.AddSeconds($ProbeTimeoutSeconds)
    $lastReadError = $null
    while ([DateTimeOffset]::UtcNow -lt $deadline) {
        if (Test-Path -LiteralPath $Path -PathType Leaf) {
            try {
                switch ($Contract) {
                    'export_terminal' {
                        return Read-StrictExportTerminalEvent -Path $Path
                    }
                    'batch_lease' {
                        return Read-StrictBatchLeaseEvent -Path $Path
                    }
                    'project_open' {
                        return Read-StrictProjectOpenEvent -Path $Path
                    }
                }
                return Read-StrictProbeEvent -Path $Path
            }
            catch {
                if ($Contract -ne 'operation') {
                    throw
                }
                $lastReadError = $_.Exception.Message
            }
        }
        $failureFiles = @(
            Get-ChildItem `
                -LiteralPath $ProbeRoot `
                -Filter 'failure-*.json' `
                -File `
                -ErrorAction SilentlyContinue
        )
        if ($failureFiles.Count -gt 0) {
            $diagnostic = Get-ProbeFailureDiagnostic -ProbeRoot $ProbeRoot
            throw "OperationGate probe reported a typed failure: $diagnostic"
        }
        foreach ($process in $Processes) {
            if ($process.HasExited) {
                $diagnostic = Get-ProbeFailureDiagnostic -ProbeRoot $ProbeRoot
                throw (
                    "OperationGate probe process $($process.Id) exited with " +
                    "code $($process.ExitCode) before '$Path' was produced. " +
                    "Probe diagnostic: $diagnostic"
                )
            }
        }
        Start-Sleep -Milliseconds 50
    }

    $diagnostic = Get-ProbeFailureDiagnostic -ProbeRoot $ProbeRoot
    throw (
        "Timed out waiting for OperationGate event '$Path'. " +
        "Last JSON read error: $lastReadError. Probe diagnostic: $diagnostic"
    )
}

function Assert-ProbeEvent {
    param(
        [Parameter(Mandatory = $true)]
        [object] $Event,
        [Parameter(Mandatory = $true)]
        [int] $ExpectedProcessId,
        [Parameter(Mandatory = $true)]
        [ValidateSet('independent', 'multiwindow')]
        [string] $ExpectedTopology,
        [Parameter(Mandatory = $true)]
        [ValidateSet('main', 'project-b')]
        [string] $ExpectedWindowLabel,
        [Parameter(Mandatory = $true)]
        [ValidateSet(
            'owner_ready',
            'challenger_conflict',
            'owner_released',
            'challenger_success'
        )]
        [string] $ExpectedState
    )

    if (
        [int] $Event.schemaVersion -ne 1 -or
        [long] $Event.processId -ne $ExpectedProcessId -or
        [string] $Event.topology -ne $ExpectedTopology -or
        [string] $Event.windowLabel -ne $ExpectedWindowLabel -or
        [string] $Event.state -ne $ExpectedState -or
        [string] $Event.operationMode -ne 'normal_export'
    ) {
        $actual = $Event | ConvertTo-Json -Depth 5 -Compress
        throw (
            "Invalid OperationGate event for state '$ExpectedState'. " +
            "Received: $actual"
        )
    }

    return [ordered]@{
        schemaVersion = 1
        processId = [int] $Event.processId
        topology = [string] $Event.topology
        windowLabel = [string] $Event.windowLabel
        state = [string] $Event.state
        operationMode = [string] $Event.operationMode
    }
}

function Open-ProbeMarker {
    param(
        [Parameter(Mandatory = $true)]
        [string] $ProbeRoot,
        [Parameter(Mandatory = $true)]
        [ValidatePattern('^[a-z][a-z0-9-]{0,63}$')]
        [string] $Name
    )

    $path = Join-Path $ProbeRoot $Name
    $stream = [System.IO.File]::Open(
        $path,
        [System.IO.FileMode]::CreateNew,
        [System.IO.FileAccess]::Write,
        [System.IO.FileShare]::Read
    )
    try {
        $bytes = [System.Text.Encoding]::UTF8.GetBytes("release`n")
        $stream.Write($bytes, 0, $bytes.Length)
        $stream.Flush($true)
    }
    finally {
        $stream.Dispose()
    }
    return $path
}

function Open-OwnerReleaseGate {
    param([Parameter(Mandatory = $true)][string] $ProbeRoot)

    return Open-ProbeMarker -ProbeRoot $ProbeRoot -Name 'release-owner'
}

function Assert-ExportTerminalEvent {
    param(
        [Parameter(Mandatory = $true)]
        [object] $Event,
        [Parameter(Mandatory = $true)]
        [int] $ExpectedProcessId,
        [Parameter(Mandatory = $true)]
        [ValidateSet('independent', 'multiwindow')]
        [string] $ExpectedTopology,
        [Parameter(Mandatory = $true)]
        [ValidateSet('main', 'project-b')]
        [string] $ExpectedWindowLabel,
        [Parameter(Mandatory = $true)]
        [ValidateSet('success', 'failure', 'cancellation', 'owner_death')]
        [string] $ExpectedScenario,
        [Parameter(Mandatory = $true)]
        [ValidateSet(
            'owner_ready',
            'challenger_conflict',
            'owner_terminal',
            'challenger_success'
        )]
        [string] $ExpectedState
    )

    if (
        [int] $Event.schemaVersion -ne 1 -or
        [long] $Event.processId -ne $ExpectedProcessId -or
        [string] $Event.topology -cne $ExpectedTopology -or
        [string] $Event.windowLabel -cne $ExpectedWindowLabel -or
        [string] $Event.scenario -cne $ExpectedScenario -or
        [string] $Event.state -cne $ExpectedState -or
        [string] $Event.operationMode -cne 'normal_export'
    ) {
        $actual = $Event | ConvertTo-Json -Depth 5 -Compress
        throw (
            "Invalid Export terminal event for state '$ExpectedState'. " +
            "Received: $actual"
        )
    }

    $fullResources = @(
        'operation_gate',
        'cache_pause',
        'processor_reservation'
    )
    $resources = @($Event.resources)
    $progressStages = @($Event.progressStages)
    $hasOperation = -not [string]::IsNullOrWhiteSpace(
        [string] $Event.operationId
    )
    switch ($ExpectedState) {
        'owner_ready' {
            if (
                $null -ne $Event.terminal -or
                -not $hasOperation -or
                $progressStages -cnotcontains 'preparing' -or
                $null -ne $Event.cancellationDisposition -or
                ($resources -join "`0") -cne ($fullResources -join "`0") -or
                [string] $Event.resourceState -cne 'held' -or
                $null -ne $Event.outputBytes
            ) {
                throw 'The owner_ready event does not prove a complete held lease.'
            }
        }
        'challenger_conflict' {
            if (
                [string] $Event.terminal -cne 'conflict' -or
                $hasOperation -or
                $progressStages.Count -ne 0 -or
                $null -ne $Event.cancellationDisposition -or
                $resources.Count -ne 0 -or
                [string] $Event.resourceState -cne 'blocked' -or
                $null -ne $Event.outputBytes
            ) {
                throw 'The challenger_conflict event is not a typed immediate conflict.'
            }
        }
        'owner_terminal' {
            $expectedTerminal = switch ($ExpectedScenario) {
                'success' { 'success' }
                'failure' { 'failed' }
                'cancellation' { 'cancelled' }
                default { throw 'owner_death must not emit owner_terminal.' }
            }
            if (
                [string] $Event.terminal -cne $expectedTerminal -or
                -not $hasOperation -or
                $progressStages -cnotcontains 'preparing' -or
                $resources.Count -ne 0 -or
                [string] $Event.resourceState -cne 'released'
            ) {
                throw 'The owner_terminal event does not prove the expected release.'
            }
            if ($ExpectedScenario -eq 'success') {
                if (
                    $progressStages -cnotcontains 'completed' -or
                    [long] $Event.outputBytes -le 0 -or
                    $null -ne $Event.cancellationDisposition
                ) {
                    throw 'The successful owner did not prove a published output.'
                }
            }
            elseif ($ExpectedScenario -eq 'failure') {
                if (
                    $progressStages -ccontains 'completed' -or
                    $null -ne $Event.outputBytes -or
                    $null -ne $Event.cancellationDisposition
                ) {
                    throw 'The failed owner exposed a successful publication.'
                }
            }
            else {
                if (
                    [string] $Event.cancellationDisposition -cne 'requested' -or
                    $progressStages -ccontains 'publishing' -or
                    $progressStages -ccontains 'completed' -or
                    $null -ne $Event.outputBytes
                ) {
                    throw 'The cancelled owner crossed the publication boundary.'
                }
            }
        }
        'challenger_success' {
            if (
                [string] $Event.terminal -cne 'success' -or
                -not $hasOperation -or
                $progressStages -cnotcontains 'preparing' -or
                $progressStages -cnotcontains 'completed' -or
                $null -ne $Event.cancellationDisposition -or
                ($resources -join "`0") -cne ($fullResources -join "`0") -or
                [string] $Event.resourceState -cne 'reacquired' -or
                [long] $Event.outputBytes -le 0
            ) {
                throw 'The challenger did not prove a complete lease reacquisition.'
            }
        }
    }

    return [ordered]@{
        schemaVersion = 1
        processId = [int] $Event.processId
        topology = [string] $Event.topology
        windowLabel = [string] $Event.windowLabel
        scenario = [string] $Event.scenario
        state = [string] $Event.state
        operationMode = [string] $Event.operationMode
        operationId = $Event.operationId
        terminal = $Event.terminal
        progressStages = @($progressStages)
        cancellationDisposition = $Event.cancellationDisposition
        resources = @($resources)
        resourceState = [string] $Event.resourceState
        outputBytes = $Event.outputBytes
    }
}

function Assert-BatchProgressOrder {
    param(
        [Parameter(Mandatory = $true)]
        [string[]] $ProgressStages,
        [Parameter(Mandatory = $true)]
        [ValidateSet(
            'success',
            'before_preparation',
            'between_promotions'
        )]
        [string] $Scenario
    )

    foreach ($stage in $ProgressStages) {
        if (
            $stage -cnotmatch
                '^[01]:(preparing|loading_sources|composing|encoding_output|verifying|publishing|completed)$'
        ) {
            throw "The Batch lease reported an unknown progress stage '$stage'."
        }
    }
    if (
        $ProgressStages -cnotcontains '0:preparing' -or
        $ProgressStages -cnotcontains '0:publishing'
    ) {
        throw 'The Batch lease did not report the first item boundaries.'
    }

    if ($Scenario -eq 'between_promotions') {
        if (
            @($ProgressStages | Where-Object { $_ -notlike '0:*' }).Count -gt 0 -or
            $ProgressStages -ccontains '0:completed'
        ) {
            throw 'The partial promotion exposed an impossible completed item.'
        }
        return
    }

    $firstSecondItem = -1
    for ($index = 0; $index -lt $ProgressStages.Count; $index++) {
        if ($ProgressStages[$index] -clike '1:*') {
            $firstSecondItem = $index
            break
        }
    }
    $firstCompleted = [Array]::IndexOf($ProgressStages, '0:completed')
    if (
        $firstSecondItem -lt 0 -or
        $firstCompleted -lt 0 -or
        $firstSecondItem -le $firstCompleted -or
        @($ProgressStages[$firstSecondItem..($ProgressStages.Count - 1)] |
            Where-Object { $_ -clike '0:*' }).Count -gt 0
    ) {
        throw 'The Batch lease progress does not prove serial item execution.'
    }
    if ($ProgressStages -cnotcontains '1:preparing') {
        throw 'The Batch lease never began its second item.'
    }
    if ($Scenario -eq 'success') {
        if (
            $ProgressStages -cnotcontains '1:publishing' -or
            $ProgressStages -cnotcontains '1:completed'
        ) {
            throw 'The successful Batch lease did not complete its second item.'
        }
    }
    elseif (
        $ProgressStages -ccontains '1:publishing' -or
        $ProgressStages -ccontains '1:completed'
    ) {
        throw 'The preparation failure crossed the publication boundary.'
    }
}

function Assert-BatchLeaseEvent {
    param(
        [Parameter(Mandatory = $true)]
        [object] $Event,
        [Parameter(Mandatory = $true)]
        [int] $ExpectedProcessId,
        [Parameter(Mandatory = $true)]
        [string] $ExpectedScenario,
        [Parameter(Mandatory = $true)]
        [ValidateSet(
            'owner_ready',
            'challenger_conflict',
            'between_items_ready',
            'between_items_conflict',
            'owner_terminal',
            'successor_success'
        )]
        [string] $ExpectedState
    )

    Assert-BatchScenarioName -Scenario $ExpectedScenario

    $expectedTotalItems = if (
        $ExpectedScenario -eq 'success' -or
        $ExpectedScenario -eq 'before_preparation'
    ) {
        2
    }
    else {
        1
    }
    if (
        [int] $Event.schemaVersion -ne 1 -or
        [long] $Event.processId -ne $ExpectedProcessId -or
        [string] $Event.scenario -cne $ExpectedScenario -or
        [string] $Event.state -cne $ExpectedState -or
        [int] $Event.totalItems -ne $expectedTotalItems -or
        [int] $Event.completedItems -gt $expectedTotalItems
    ) {
        $actual = $Event | ConvertTo-Json -Depth 8 -Compress
        throw (
            "Invalid Batch lease event for state '$ExpectedState'. " +
            "Received: $actual"
        )
    }

    $resources = @($Event.resources)
    $progressStages = @($Event.progressStages)
    $outputEvidence = @($Event.outputEvidence)
    $fullResources = @(
        'operation_gate',
        'cache_pause',
        'processor_reservation'
    )
    $hasOperation = -not [string]::IsNullOrWhiteSpace(
        [string] $Event.operationId
    )
    switch ($ExpectedState) {
        'owner_ready' {
            if (
                [string] $Event.role -cne 'owner' -or
                [string] $Event.operationMode -cne 'batch_exclusive' -or
                -not $hasOperation -or
                $null -ne $Event.terminal -or
                $null -ne $Event.itemIndex -or
                [int] $Event.completedItems -ne 0 -or
                $null -ne $Event.promotedOutputs -or
                $null -ne $Event.totalOutputs -or
                $null -ne $Event.failureStage -or
                $progressStages.Count -ne 0 -or
                ($resources -join "`0") -cne ($fullResources -join "`0") -or
                [string] $Event.resourceState -cne 'held' -or
                $outputEvidence.Count -ne 0 -or
                $null -ne $Event.outputBytes
            ) {
                throw 'The owner_ready event does not prove one complete BatchExclusive lease.'
            }
        }
        'challenger_conflict' {
            if (
                [string] $Event.role -cne 'challenger' -or
                [string] $Event.operationMode -cne 'normal_export' -or
                $hasOperation -or
                [string] $Event.terminal -cne 'conflict' -or
                $null -ne $Event.itemIndex -or
                [int] $Event.completedItems -ne 0 -or
                $null -ne $Event.promotedOutputs -or
                $null -ne $Event.totalOutputs -or
                $null -ne $Event.failureStage -or
                $progressStages.Count -ne 0 -or
                $resources.Count -ne 0 -or
                [string] $Event.resourceState -cne 'blocked' -or
                $outputEvidence.Count -ne 0 -or
                $null -ne $Event.outputBytes
            ) {
                throw 'The first batch challenger was not blocked before work began.'
            }
        }
        'between_items_ready' {
            if (
                [string] $Event.role -cne 'owner' -or
                [string] $Event.operationMode -cne 'batch_exclusive' -or
                -not $hasOperation -or
                $null -ne $Event.terminal -or
                $null -eq $Event.itemIndex -or
                [int] $Event.itemIndex -ne 0 -or
                [int] $Event.completedItems -ne 1 -or
                $null -ne $Event.promotedOutputs -or
                $null -ne $Event.totalOutputs -or
                $null -ne $Event.failureStage -or
                $progressStages.Count -ne 0 -or
                ($resources -join "`0") -cne ($fullResources -join "`0") -or
                [string] $Event.resourceState -cne 'held' -or
                $outputEvidence.Count -ne 0 -or
                $null -ne $Event.outputBytes
            ) {
                throw 'The between-items event does not prove a continuous held lease.'
            }
        }
        'between_items_conflict' {
            if (
                [string] $Event.role -cne 'challenger' -or
                [string] $Event.operationMode -cne 'normal_export' -or
                $hasOperation -or
                [string] $Event.terminal -cne 'conflict' -or
                $null -eq $Event.itemIndex -or
                [int] $Event.itemIndex -ne 0 -or
                [int] $Event.completedItems -ne 1 -or
                $null -ne $Event.promotedOutputs -or
                $null -ne $Event.totalOutputs -or
                $null -ne $Event.failureStage -or
                $progressStages.Count -ne 0 -or
                $resources.Count -ne 0 -or
                [string] $Event.resourceState -cne 'blocked' -or
                $outputEvidence.Count -ne 0 -or
                $null -ne $Event.outputBytes
            ) {
                throw 'The normal export entered between two batch items.'
            }
        }
        'owner_terminal' {
            if ($ExpectedScenario -eq 'owner_death') {
                throw 'owner_death must not emit owner_terminal.'
            }
            $expectedTerminal = if ($ExpectedScenario -eq 'success') {
                'success'
            }
            else {
                'failed'
            }
            if (
                [string] $Event.role -cne 'owner' -or
                [string] $Event.operationMode -cne 'batch_exclusive' -or
                -not $hasOperation -or
                [string] $Event.terminal -cne $expectedTerminal -or
                ($resources -join "`0") -cne ($fullResources -join "`0") -or
                [string] $Event.resourceState -cne 'released' -or
                $null -ne $Event.outputBytes
            ) {
                throw 'The batch owner did not publish its expected terminal state.'
            }
            switch ($ExpectedScenario) {
                'success' {
                    if (
                        $null -ne $Event.itemIndex -or
                        [int] $Event.completedItems -ne 2 -or
                        $null -eq $Event.promotedOutputs -or
                        [int] $Event.promotedOutputs -ne 2 -or
                        $null -eq $Event.totalOutputs -or
                        [int] $Event.totalOutputs -ne 2 -or
                        $null -ne $Event.failureStage -or
                        $outputEvidence.Count -ne 2
                    ) {
                        throw 'The successful batch terminal does not prove two outputs.'
                    }
                    Assert-BatchProgressOrder `
                        -ProgressStages $progressStages `
                        -Scenario $ExpectedScenario
                }
                'before_preparation' {
                    if (
                        $null -eq $Event.itemIndex -or
                        [int] $Event.itemIndex -ne 1 -or
                        [int] $Event.completedItems -ne 1 -or
                        $null -eq $Event.promotedOutputs -or
                        [int] $Event.promotedOutputs -ne 0 -or
                        $null -eq $Event.totalOutputs -or
                        [int] $Event.totalOutputs -ne 1 -or
                        [string] $Event.failureStage -cne 'prepare_output' -or
                        $outputEvidence.Count -ne 1
                    ) {
                        throw 'The injected preparation failure crossed its expected boundary.'
                    }
                    Assert-BatchProgressOrder `
                        -ProgressStages $progressStages `
                        -Scenario $ExpectedScenario
                }
                'between_promotions' {
                    if (
                        $null -eq $Event.itemIndex -or
                        [int] $Event.itemIndex -ne 0 -or
                        [int] $Event.completedItems -ne 0 -or
                        $null -eq $Event.promotedOutputs -or
                        [int] $Event.promotedOutputs -ne 1 -or
                        $null -eq $Event.totalOutputs -or
                        [int] $Event.totalOutputs -ne 2 -or
                        [string] $Event.failureStage -cne 'publish_output' -or
                        $outputEvidence.Count -ne 1
                    ) {
                        throw 'The partial publication was not typed as one of two promotions.'
                    }
                    Assert-BatchProgressOrder `
                        -ProgressStages $progressStages `
                        -Scenario $ExpectedScenario
                }
            }
        }
        'successor_success' {
            $expectedRole = if ($ExpectedScenario -eq 'owner_death') {
                'challenger'
            }
            else {
                'owner'
            }
            $expectedCompleted = switch ($ExpectedScenario) {
                'success' { 2 }
                'before_preparation' { 1 }
                default { 0 }
            }
            if (
                [string] $Event.role -cne $expectedRole -or
                [string] $Event.operationMode -cne 'normal_export' -or
                $hasOperation -or
                [string] $Event.terminal -cne 'success' -or
                $null -ne $Event.itemIndex -or
                [int] $Event.completedItems -ne $expectedCompleted -or
                $null -ne $Event.promotedOutputs -or
                $null -ne $Event.totalOutputs -or
                $null -ne $Event.failureStage -or
                $progressStages.Count -ne 0 -or
                ($resources -join "`0") -cne ($fullResources -join "`0") -or
                [string] $Event.resourceState -cne 'reacquired' -or
                $outputEvidence.Count -ne 0 -or
                [long] $Event.outputBytes -le 0
            ) {
                throw 'A real normal export did not reacquire the released lease.'
            }
        }
    }

    return [ordered]@{
        schemaVersion = 1
        processId = [int] $Event.processId
        role = [string] $Event.role
        scenario = [string] $Event.scenario
        state = [string] $Event.state
        operationMode = [string] $Event.operationMode
        operationId = $Event.operationId
        terminal = $Event.terminal
        itemIndex = $Event.itemIndex
        totalItems = [int] $Event.totalItems
        completedItems = [int] $Event.completedItems
        promotedOutputs = $Event.promotedOutputs
        totalOutputs = $Event.totalOutputs
        failureStage = $Event.failureStage
        progressStages = @($progressStages)
        resources = @($resources)
        resourceState = [string] $Event.resourceState
        outputEvidence = @($outputEvidence)
        outputBytes = $Event.outputBytes
    }
}

function Invoke-OperationTopologyProbe {
    param(
        [Parameter(Mandatory = $true)]
        [ValidateSet('independent', 'multiwindow')]
        [string] $Topology,
        [Parameter(Mandatory = $true)]
        [string] $ProbeRoot
    )

    $scenarioProcesses =
        [System.Collections.Generic.List[System.Diagnostics.Process]]::new()
    $startedAt = [DateTimeOffset]::UtcNow
    $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    try {
        if ($Topology -eq 'independent') {
            $ownerProcess = Start-OperationProbeProcess `
                -Topology $Topology `
                -ProbeRoot $ProbeRoot `
                -ProjectSlot 'a'
            $scenarioProcesses.Add($ownerProcess)
            $ownerReadyRaw = Wait-ForProbeEvent `
                -Path (Join-Path $ProbeRoot 'owner-ready.json') `
                -Processes @($ownerProcess) `
                -ProbeRoot $ProbeRoot

            $challengerProcess = Start-OperationProbeProcess `
                -Topology $Topology `
                -ProbeRoot $ProbeRoot `
                -ProjectSlot 'b'
            $scenarioProcesses.Add($challengerProcess)
            $processes = @($ownerProcess, $challengerProcess)
            $ownerWindow = 'main'
            $challengerWindow = 'main'
        }
        else {
            $ownerProcess = Start-OperationProbeProcess `
                -Topology $Topology `
                -ProbeRoot $ProbeRoot
            $scenarioProcesses.Add($ownerProcess)
            $challengerProcess = $ownerProcess
            $processes = @($ownerProcess)
            $ownerWindow = 'main'
            $challengerWindow = 'project-b'
            $ownerReadyRaw = Wait-ForProbeEvent `
                -Path (Join-Path $ProbeRoot 'owner-ready.json') `
                -Processes $processes `
                -ProbeRoot $ProbeRoot
        }

        $ownerReady = Assert-ProbeEvent `
            -Event $ownerReadyRaw `
            -ExpectedProcessId $ownerProcess.Id `
            -ExpectedTopology $Topology `
            -ExpectedWindowLabel $ownerWindow `
            -ExpectedState 'owner_ready'
        $challengerConflict = Assert-ProbeEvent `
            -Event (Wait-ForProbeEvent `
                -Path (Join-Path $ProbeRoot 'challenger-conflict.json') `
                -Processes $processes `
                -ProbeRoot $ProbeRoot) `
            -ExpectedProcessId $challengerProcess.Id `
            -ExpectedTopology $Topology `
            -ExpectedWindowLabel $challengerWindow `
            -ExpectedState 'challenger_conflict'

        $releasePath = Open-OwnerReleaseGate -ProbeRoot $ProbeRoot
        $ownerReleased = Assert-ProbeEvent `
            -Event (Wait-ForProbeEvent `
                -Path (Join-Path $ProbeRoot 'owner-released.json') `
                -Processes $processes `
                -ProbeRoot $ProbeRoot) `
            -ExpectedProcessId $ownerProcess.Id `
            -ExpectedTopology $Topology `
            -ExpectedWindowLabel $ownerWindow `
            -ExpectedState 'owner_released'
        $challengerSuccess = Assert-ProbeEvent `
            -Event (Wait-ForProbeEvent `
                -Path (Join-Path $ProbeRoot 'challenger-success.json') `
                -Processes $processes `
                -ProbeRoot $ProbeRoot) `
            -ExpectedProcessId $challengerProcess.Id `
            -ExpectedTopology $Topology `
            -ExpectedWindowLabel $challengerWindow `
            -ExpectedState 'challenger_success'

        $stopwatch.Stop()
        return [ordered]@{
            topology = $Topology
            passed = $true
            startedAtUtc = $startedAt.ToString('o')
            elapsedMs = [long] $stopwatch.ElapsedMilliseconds
            processCount = $scenarioProcesses.Count
            processIds = @(
                $scenarioProcesses |
                    ForEach-Object { $_.Id }
            )
            releaseGateCreated = Test-Path -LiteralPath $releasePath -PathType Leaf
            ownerReady = $ownerReady
            challengerConflict = $challengerConflict
            ownerReleased = $ownerReleased
            challengerSuccess = $challengerSuccess
        }
    }
    finally {
        $stopwatch.Stop()
        foreach ($process in @($scenarioProcesses)) {
            Stop-OwnedOperationProbeProcess -Process $process
        }
    }
}

function Invoke-ExportTerminalScenario {
    param(
        [Parameter(Mandatory = $true)]
        [ValidateSet('independent', 'multiwindow')]
        [string] $Topology,
        [Parameter(Mandatory = $true)]
        [ValidateSet('success', 'failure', 'cancellation', 'owner_death')]
        [string] $Scenario,
        [Parameter(Mandatory = $true)]
        [string] $ProbeRoot
    )

    $scenarioProcesses =
        [System.Collections.Generic.List[System.Diagnostics.Process]]::new()
    $sidecarBackupPath = Join-Path $ProbeRoot 'myalbuns-imaging.exe.disabled'
    $sidecarWasMoved = $false
    $startedAt = [DateTimeOffset]::UtcNow
    $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    try {
        if ($Scenario -eq 'failure') {
            $releaseDirectory = [System.IO.Path]::GetFullPath(
                (Join-Path $targetDirectory 'release')
            )
            $verifiedSidecarPath = [System.IO.Path]::GetFullPath(
                $imagingExecutablePath
            )
            if (
                -not (Test-Path -LiteralPath $verifiedSidecarPath -PathType Leaf) -or
                -not [string]::Equals(
                    [System.IO.Path]::GetDirectoryName($verifiedSidecarPath),
                    $releaseDirectory,
                    [System.StringComparison]::OrdinalIgnoreCase
                )
            ) {
                throw 'The failure probe sidecar is not the verified release artifact.'
            }
            Move-Item `
                -LiteralPath $verifiedSidecarPath `
                -Destination $sidecarBackupPath
            $sidecarWasMoved = $true
        }

        if ($Topology -eq 'independent') {
            $ownerProcess = Start-OperationProbeProcess `
                -Topology $Topology `
                -ProbeRoot $ProbeRoot `
                -ProjectSlot 'a' `
                -TerminalScenario $Scenario
            $scenarioProcesses.Add($ownerProcess)
            $ownerReadyRaw = Wait-ForProbeEvent `
                -Path (Join-Path $ProbeRoot 'owner-ready.json') `
                -Processes @($ownerProcess) `
                -ProbeRoot $ProbeRoot `
                -Contract 'export_terminal'

            $challengerProcess = Start-OperationProbeProcess `
                -Topology $Topology `
                -ProbeRoot $ProbeRoot `
                -ProjectSlot 'b' `
                -TerminalScenario $Scenario
            $scenarioProcesses.Add($challengerProcess)
            $processes = @($ownerProcess, $challengerProcess)
            $ownerWindow = 'main'
            $challengerWindow = 'main'
        }
        else {
            $ownerProcess = Start-OperationProbeProcess `
                -Topology $Topology `
                -ProbeRoot $ProbeRoot `
                -TerminalScenario $Scenario
            $scenarioProcesses.Add($ownerProcess)
            $challengerProcess = $ownerProcess
            $processes = @($ownerProcess)
            $ownerWindow = 'main'
            $challengerWindow = 'project-b'
            $ownerReadyRaw = Wait-ForProbeEvent `
                -Path (Join-Path $ProbeRoot 'owner-ready.json') `
                -Processes $processes `
                -ProbeRoot $ProbeRoot `
                -Contract 'export_terminal'
        }

        $ownerProcessId = $ownerProcess.Id
        $ownerReady = Assert-ExportTerminalEvent `
            -Event $ownerReadyRaw `
            -ExpectedProcessId $ownerProcessId `
            -ExpectedTopology $Topology `
            -ExpectedWindowLabel $ownerWindow `
            -ExpectedScenario $Scenario `
            -ExpectedState 'owner_ready'
        $challengerConflict = Assert-ExportTerminalEvent `
            -Event (Wait-ForProbeEvent `
                -Path (Join-Path $ProbeRoot 'challenger-conflict.json') `
                -Processes $processes `
                -ProbeRoot $ProbeRoot `
                -Contract 'export_terminal') `
            -ExpectedProcessId $challengerProcess.Id `
            -ExpectedTopology $Topology `
            -ExpectedWindowLabel $challengerWindow `
            -ExpectedScenario $Scenario `
            -ExpectedState 'challenger_conflict'

        $ownerTerminal = $null
        $ownerTerminated = $false
        if ($Scenario -eq 'owner_death') {
            Stop-OwnedOperationProbeProcess `
                -Process $ownerProcess `
                -RequireLiveKill
            [void] $scenarioProcesses.Remove($ownerProcess)
            $ownerTerminated = $true

            if ($Topology -eq 'multiwindow') {
                $challengerProcess = Start-OperationProbeProcess `
                    -Topology $Topology `
                    -ProbeRoot $ProbeRoot `
                    -TerminalScenario $Scenario `
                    -TerminalPhase 'successor'
                $scenarioProcesses.Add($challengerProcess)
            }
            $processes = @($challengerProcess)
            [void] (Open-ProbeMarker `
                -ProbeRoot $ProbeRoot `
                -Name 'allow-successor')
        }
        else {
            $trigger = if ($Scenario -eq 'cancellation') {
                'cancel-owner'
            }
            else {
                'continue-owner'
            }
            [void] (Open-ProbeMarker -ProbeRoot $ProbeRoot -Name $trigger)
            $ownerTerminal = Assert-ExportTerminalEvent `
                -Event (Wait-ForProbeEvent `
                    -Path (Join-Path $ProbeRoot 'owner-terminal.json') `
                    -Processes $processes `
                    -ProbeRoot $ProbeRoot `
                    -Contract 'export_terminal') `
                -ExpectedProcessId $ownerProcessId `
                -ExpectedTopology $Topology `
                -ExpectedWindowLabel $ownerWindow `
                -ExpectedScenario $Scenario `
                -ExpectedState 'owner_terminal'

            if ($sidecarWasMoved) {
                Move-Item `
                    -LiteralPath $sidecarBackupPath `
                    -Destination $imagingExecutablePath
                $sidecarWasMoved = $false
            }
            [void] (Open-ProbeMarker `
                -ProbeRoot $ProbeRoot `
                -Name 'allow-successor')
        }

        $challengerSuccess = Assert-ExportTerminalEvent `
            -Event (Wait-ForProbeEvent `
                -Path (Join-Path $ProbeRoot 'challenger-success.json') `
                -Processes $processes `
                -ProbeRoot $ProbeRoot `
                -Contract 'export_terminal') `
            -ExpectedProcessId $challengerProcess.Id `
            -ExpectedTopology $Topology `
            -ExpectedWindowLabel $challengerWindow `
            -ExpectedScenario $Scenario `
            -ExpectedState 'challenger_success'

        $stopwatch.Stop()
        return [ordered]@{
            topology = $Topology
            scenario = $Scenario
            passed = $true
            startedAtUtc = $startedAt.ToString('o')
            elapsedMs = [long] $stopwatch.ElapsedMilliseconds
            ownerProcessId = $ownerProcessId
            successorProcessId = [int] $challengerProcess.Id
            ownerTerminated = $ownerTerminated
            ownerReady = $ownerReady
            challengerConflict = $challengerConflict
            ownerTerminal = $ownerTerminal
            challengerSuccess = $challengerSuccess
        }
    }
    finally {
        $stopwatch.Stop()
        try {
            foreach ($process in @($scenarioProcesses)) {
                Stop-OwnedOperationProbeProcess -Process $process
            }
        }
        finally {
            if ($sidecarWasMoved) {
                if (Test-Path -LiteralPath $imagingExecutablePath) {
                    throw 'Refusing to overwrite the restored imaging sidecar.'
                }
                Move-Item `
                    -LiteralPath $sidecarBackupPath `
                    -Destination $imagingExecutablePath
            }
        }
    }
}

function Assert-BatchOutputEvidence {
    param(
        [Parameter(Mandatory = $true)]
        [object[]] $Evidence,
        [Parameter(Mandatory = $true)]
        [string[]] $ExpectedPaths
    )

    if ($Evidence.Count -ne $ExpectedPaths.Count) {
        throw 'The Batch lease output evidence count differs from the filesystem.'
    }
    $verified = @(
        foreach ($path in $ExpectedPaths) {
            if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
                throw "The expected Batch lease output '$path' does not exist."
            }
            $item = Get-Item -LiteralPath $path
            $name = $item.Name
            $matches = @($Evidence | Where-Object { $_.name -ceq $name })
            if ($matches.Count -ne 1) {
                throw "The Batch lease evidence does not identify '$name' exactly once."
            }
            $sha256 = (
                Get-FileHash -LiteralPath $path -Algorithm SHA256
            ).Hash.ToLowerInvariant()
            if (
                [long] $matches[0].bytes -ne [long] $item.Length -or
                [string] $matches[0].sha256 -cne $sha256
            ) {
                throw "The Batch lease evidence does not match '$name' byte-for-byte."
            }
            [ordered]@{
                name = $name
                bytes = [long] $item.Length
                sha256 = $sha256
            }
        }
    )
    return @($verified)
}

function Assert-NoBatchPreparationRemnants {
    param([Parameter(Mandatory = $true)][string] $ProbeRoot)

    $remnants = @(
        Get-ChildItem `
            -LiteralPath $ProbeRoot `
            -Recurse `
            -Force `
            -ErrorAction SilentlyContinue |
            Where-Object { $_.Name -like '.myalbuns-export-*.tmp' }
    )
    if ($remnants.Count -gt 0) {
        throw (
            'The Batch lease left preparation remnants: ' +
            (($remnants | ForEach-Object { $_.FullName }) -join ', ')
        )
    }
}

function Invoke-BatchLeaseScenario {
    param(
        [Parameter(Mandatory = $true)]
        [string] $Scenario,
        [Parameter(Mandatory = $true)]
        [string] $ProbeRoot
    )

    Assert-BatchScenarioName -Scenario $Scenario

    $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    $startedAt = [DateTimeOffset]::UtcNow
    $scenarioProcesses =
        [System.Collections.Generic.List[System.Diagnostics.Process]]::new()
    $lockedOutput = $null
    $ownerTerminated = $false
    $ownerTerminal = $null
    $betweenItemsReady = $null
    $betweenItemsConflict = $null
    $previousOutputs = @()
    try {
        $ownerProcess = Start-OperationProbeProcess `
            -Topology 'independent' `
            -ProbeRoot $ProbeRoot `
            -ProjectSlot 'a' `
            -BatchScenario $Scenario
        $scenarioProcesses.Add($ownerProcess)
        $challengerProcess = Start-OperationProbeProcess `
            -Topology 'independent' `
            -ProbeRoot $ProbeRoot `
            -ProjectSlot 'b' `
            -BatchScenario $Scenario
        $scenarioProcesses.Add($challengerProcess)
        $processes = @($ownerProcess, $challengerProcess)

        $ownerReady = Assert-BatchLeaseEvent `
            -Event (Wait-ForProbeEvent `
                -Path (Join-Path $ProbeRoot 'owner-ready.json') `
                -Processes $processes `
                -ProbeRoot $ProbeRoot `
                -Contract 'batch_lease') `
            -ExpectedProcessId $ownerProcess.Id `
            -ExpectedScenario $Scenario `
            -ExpectedState 'owner_ready'
        $challengerConflict = Assert-BatchLeaseEvent `
            -Event (Wait-ForProbeEvent `
                -Path (Join-Path $ProbeRoot 'challenger-conflict.json') `
                -Processes $processes `
                -ProbeRoot $ProbeRoot `
                -Contract 'batch_lease') `
            -ExpectedProcessId $challengerProcess.Id `
            -ExpectedScenario $Scenario `
            -ExpectedState 'challenger_conflict'

        if ($Scenario -eq 'between_promotions') {
            $firstOutput = Join-Path `
                $ProbeRoot `
                'destination\between-promotions-1.png'
            $secondOutput = Join-Path `
                $ProbeRoot `
                'destination\between-promotions-2.png'
            foreach ($path in @($firstOutput, $secondOutput)) {
                if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
                    throw "The partial-promotion fixture '$path' was not created."
                }
            }
            $previousOutputs = @(
                foreach ($path in @($firstOutput, $secondOutput)) {
                    [ordered]@{
                        name = (Get-Item -LiteralPath $path).Name
                        bytes = [long] (Get-Item -LiteralPath $path).Length
                        sha256 = (
                            Get-FileHash -LiteralPath $path -Algorithm SHA256
                        ).Hash.ToLowerInvariant()
                    }
                }
            )
            # The exclusive handle deterministically blocks replacement of the
            # second final file. The probe reports the first promoted output;
            # this harness verifies the still-locked second output afterwards.
            $lockedOutput = [System.IO.File]::Open(
                $secondOutput,
                [System.IO.FileMode]::Open,
                [System.IO.FileAccess]::Read,
                [System.IO.FileShare]::None
            )
        }

        if ($Scenario -eq 'owner_death') {
            Stop-OwnedOperationProbeProcess `
                -Process $ownerProcess `
                -RequireLiveKill
            [void] $scenarioProcesses.Remove($ownerProcess)
            $ownerTerminated = $true
            if (Test-Path -LiteralPath (Join-Path $ProbeRoot 'owner-terminal.json')) {
                throw 'The killed batch owner published a cooperative terminal event.'
            }
            [void] (Open-ProbeMarker `
                -ProbeRoot $ProbeRoot `
                -Name 'allow-successor')
        }
        else {
            [void] (Open-ProbeMarker `
                -ProbeRoot $ProbeRoot `
                -Name 'allow-batch-start')
            if (
                $Scenario -eq 'success' -or
                $Scenario -eq 'before_preparation'
            ) {
                $betweenItemsReady = Assert-BatchLeaseEvent `
                    -Event (Wait-ForProbeEvent `
                        -Path (Join-Path $ProbeRoot 'between-items-ready.json') `
                        -Processes $processes `
                        -ProbeRoot $ProbeRoot `
                        -Contract 'batch_lease') `
                    -ExpectedProcessId $ownerProcess.Id `
                    -ExpectedScenario $Scenario `
                    -ExpectedState 'between_items_ready'
                $betweenItemsConflict = Assert-BatchLeaseEvent `
                    -Event (Wait-ForProbeEvent `
                        -Path (Join-Path $ProbeRoot 'between-items-conflict.json') `
                        -Processes $processes `
                        -ProbeRoot $ProbeRoot `
                        -Contract 'batch_lease') `
                    -ExpectedProcessId $challengerProcess.Id `
                    -ExpectedScenario $Scenario `
                    -ExpectedState 'between_items_conflict'
                if (
                    [string] $betweenItemsReady.operationId -cne
                        [string] $ownerReady.operationId
                ) {
                    throw 'The BatchExclusive operation changed between items.'
                }
                [void] (Open-ProbeMarker `
                    -ProbeRoot $ProbeRoot `
                    -Name 'allow-next-item')
            }
            $ownerTerminal = Assert-BatchLeaseEvent `
                -Event (Wait-ForProbeEvent `
                    -Path (Join-Path $ProbeRoot 'owner-terminal.json') `
                    -Processes $processes `
                    -ProbeRoot $ProbeRoot `
                    -Contract 'batch_lease') `
                -ExpectedProcessId $ownerProcess.Id `
                -ExpectedScenario $Scenario `
                -ExpectedState 'owner_terminal'
            if (
                [string] $ownerTerminal.operationId -cne
                    [string] $ownerReady.operationId
            ) {
                throw 'The terminal batch event belongs to a different operation.'
            }
        }

        $successorProcessId = if ($Scenario -eq 'owner_death') {
            $challengerProcess.Id
        }
        else {
            $ownerProcess.Id
        }
        $successorProcesses = if ($Scenario -eq 'owner_death') {
            @($challengerProcess)
        }
        else {
            @($ownerProcess, $challengerProcess)
        }
        $successorSuccess = Assert-BatchLeaseEvent `
            -Event (Wait-ForProbeEvent `
                -Path (Join-Path $ProbeRoot 'successor-success.json') `
                -Processes $successorProcesses `
                -ProbeRoot $ProbeRoot `
                -Contract 'batch_lease') `
            -ExpectedProcessId $successorProcessId `
            -ExpectedScenario $Scenario `
            -ExpectedState 'successor_success'

        if ($null -ne $lockedOutput) {
            $lockedOutput.Dispose()
            $lockedOutput = $null
        }
        if (
            $Scenario -eq 'owner_death' -and
            (Test-Path -LiteralPath (Join-Path $ProbeRoot 'owner-terminal.json'))
        ) {
            throw 'The killed owner emitted a terminal event after lease recovery.'
        }

        $publicationEvidence = @()
        switch ($Scenario) {
            'success' {
                $publicationEvidence = @(
                    Assert-BatchOutputEvidence `
                        -Evidence @($ownerTerminal.outputEvidence) `
                        -ExpectedPaths @(
                            (Join-Path $ProbeRoot 'destination\success-item-1.png'),
                            (Join-Path $ProbeRoot 'destination\success-item-2.png')
                        )
                )
            }
            'before_preparation' {
                $firstOutput = Join-Path `
                    $ProbeRoot `
                    'destination\before-preparation-item-1.png'
                $secondOutput = Join-Path `
                    $ProbeRoot `
                    'destination\before-preparation-item-2.png'
                $publicationEvidence = @(
                    Assert-BatchOutputEvidence `
                        -Evidence @($ownerTerminal.outputEvidence) `
                        -ExpectedPaths @($firstOutput)
                )
                if (Test-Path -LiteralPath $secondOutput) {
                    throw 'The preparation failure published its second item.'
                }
            }
            'between_promotions' {
                $promotedEvidence = @(
                    Assert-BatchOutputEvidence `
                    -Evidence @($ownerTerminal.outputEvidence) `
                    -ExpectedPaths @($firstOutput)
                )
                if (-not (Test-Path -LiteralPath $secondOutput -PathType Leaf)) {
                    throw 'The unpromoted previous output disappeared.'
                }
                $secondItem = Get-Item -LiteralPath $secondOutput
                $currentOutputs = @(
                    $promotedEvidence[0],
                    [ordered]@{
                        name = $secondItem.Name
                        bytes = [long] $secondItem.Length
                        sha256 = (
                            Get-FileHash `
                                -LiteralPath $secondOutput `
                                -Algorithm SHA256
                        ).Hash.ToLowerInvariant()
                    }
                )
                if (
                    $currentOutputs[0].sha256 -ceq $previousOutputs[0].sha256 -or
                    $currentOutputs[1].sha256 -cne $previousOutputs[1].sha256
                ) {
                    throw 'The partial promotion did not preserve the expected old/new mix.'
                }
                $publicationEvidence = [ordered]@{
                    before = @($previousOutputs)
                    after = @($currentOutputs)
                    promotedOutputs = 1
                    totalOutputs = 2
                }
            }
            'owner_death' {
                $batchOutput = Join-Path `
                    $ProbeRoot `
                    'destination\owner-death.png'
                if (Test-Path -LiteralPath $batchOutput) {
                    throw 'The killed owner published a batch output before recovery.'
                }
            }
        }
        Assert-NoBatchPreparationRemnants -ProbeRoot $ProbeRoot

        $stopwatch.Stop()
        return [ordered]@{
            topology = 'independent'
            scenario = $Scenario
            passed = $true
            startedAtUtc = $startedAt.ToString('o')
            elapsedMs = [long] $stopwatch.ElapsedMilliseconds
            ownerProcessId = [int] $ownerReady.processId
            challengerProcessId = [int] $challengerProcess.Id
            successorProcessId = [int] $successorProcessId
            ownerTerminated = $ownerTerminated
            ownerReady = $ownerReady
            challengerConflict = $challengerConflict
            betweenItemsReady = $betweenItemsReady
            betweenItemsConflict = $betweenItemsConflict
            ownerTerminal = $ownerTerminal
            successorSuccess = $successorSuccess
            publicationEvidence = $publicationEvidence
            preparationRemnants = 0
        }
    }
    finally {
        $stopwatch.Stop()
        if ($null -ne $lockedOutput) {
            $lockedOutput.Dispose()
        }
        foreach ($process in @($scenarioProcesses)) {
            Stop-OwnedOperationProbeProcess -Process $process
        }
    }
}







try {
try {
    Set-OperationProbeEnvironmentValue `
        -Name 'CARGO_TARGET_DIR' `
        -Value $rustCheckTargetDirectory
    Set-OperationProbeEnvironmentValue `
        -Name 'CARGO_BUILD_JOBS' `
        -Value ([string] $CargoBuildJobs)
    Push-Location $script:WorkspaceRoot
    $locationWasPushed = $true
    $initialBuildInputState = Get-BuildInputState
    if ($Suite -eq 'batch') {
        Test-StrictBatchLeaseEventParser `
            -ProbeRoot $batchRoots['success']
    }
    elseif ($Suite -eq 'project_open') {
        Test-StrictProjectOpenEventParser `
            -ProbeRoot $projectOpenContext.Roots['normal_close']
    }
    else {
        Test-StrictProbeEventParser -ProbeRoot $independentRoot
        Test-StrictExportTerminalEventParser `
            -ProbeRoot $terminalRoots['independent']['success']
    }

    $rustChecks = @(
        if ($Suite -eq 'batch') {
            [ordered]@{
                name = 'operation-lease'
                arguments = @(
                    'test',
                    '-p',
                    'myalbuns-desktop',
                    '--lib',
                    'operation_lease::tests::',
                    '--',
                    '--nocapture'
                )
            }
            [ordered]@{
                name = 'batch-runner'
                arguments = @(
                    'test',
                    '-p',
                    'myalbuns-desktop',
                    '--lib',
                    'batch_runner::tests::',
                    '--',
                    '--nocapture'
                )
            }
            [ordered]@{
                name = 'grouped-export-prepares-before-publish'
                arguments = @(
                    'test',
                    '-p',
                    'myalbuns-desktop',
                    '--lib',
                    'export_pipeline::tests::grouped_export_prepares_every_output_before_publishing_the_complete_set',
                    '--',
                    '--exact',
                    '--nocapture'
                )
            }
            [ordered]@{
                name = 'grouped-export-partial-publication'
                arguments = @(
                    'test',
                    '-p',
                    'myalbuns-desktop',
                    '--lib',
                    'export_pipeline::tests::grouped_export_reports_a_typed_partial_publication_and_discards_the_remainder',
                    '--',
                    '--exact',
                    '--nocapture'
                )
            }
            [ordered]@{
                name = 'batch-lease-probe-contract'
                arguments = @(
                    'test',
                    '-p',
                    'myalbuns-desktop',
                    '--lib',
                    'batch_lease_probe::tests::',
                    '--',
                    '--nocapture'
                )
            }
        }
        elseif ($Suite -eq 'project_open') {
            Get-ProjectOpenGateRustChecks
        }
        else {
        [ordered]@{
            name = 'operation-gate'
            arguments = @(
                'test',
                '-p',
                'myalbuns-desktop',
                '--lib',
                'operation_gate::tests::',
                '--',
                '--nocapture'
            )
        },
        [ordered]@{
            name = 'operation-lease'
            arguments = @(
                'test',
                '-p',
                'myalbuns-desktop',
                '--lib',
                'operation_lease::tests::',
                '--',
                '--nocapture'
            )
        },
        [ordered]@{
            name = 'operation-gate-probe-contract'
            arguments = @(
                'test',
                '-p',
                'myalbuns-desktop',
                '--lib',
                'operation_gate_probe::tests::',
                '--',
                '--nocapture'
            )
        },
        [ordered]@{
            name = 'export-terminal-probe-contract'
            arguments = @(
                'test',
                '-p',
                'myalbuns-desktop',
                '--lib',
                'export_terminal_probe::tests::',
                '--',
                '--nocapture'
            )
        },
        [ordered]@{
            name = 'cache-pause'
            arguments = @(
                'test',
                '-p',
                'myalbuns-desktop',
                '--lib',
                'cache_engine::tests::pause_waits_for_active_work_and_blocks_new_work_until_release',
                '--',
                '--exact',
                '--nocapture'
            )
        },
        [ordered]@{
            name = 'processor-reservation'
            arguments = @(
                'test',
                '-p',
                'myalbuns-desktop',
                '--lib',
                'imaging_processor::tests::processor_reservation_serializes_callers_and_is_released_with_its_guard',
                '--',
                '--exact',
                '--nocapture'
            )
        },
        [ordered]@{
            name = 'real-owner-process-death'
            arguments = @(
                'test',
                '-p',
                'myalbuns-desktop',
                '--lib',
                'operation_gate::tests::independent_hosts_share_one_grant_and_recover_after_owner_process_termination',
                '--',
                '--ignored',
                '--exact',
                '--nocapture'
            )
        },
        [ordered]@{
            name = 'export-success-and-progress'
            arguments = @(
                'test',
                '-p',
                'myalbuns-desktop',
                '--lib',
                'export_pipeline::tests::a_verified_preparation_is_published_only_after_the_response_is_validated',
                '--',
                '--exact',
                '--nocapture'
            )
        },
        [ordered]@{
            name = 'export-failure'
            arguments = @(
                'test',
                '-p',
                'myalbuns-desktop',
                '--lib',
                'export_pipeline::tests::a_processor_crash_is_not_retried_and_preserves_the_previous_output',
                '--',
                '--exact',
                '--nocapture'
            )
        },
        [ordered]@{
            name = 'export-cancellation-before-start'
            arguments = @(
                'test',
                '-p',
                'myalbuns-desktop',
                '--lib',
                'export_pipeline::tests::cancellation_before_execution_creates_no_preparation_or_processor_work',
                '--',
                '--exact',
                '--nocapture'
            )
        },
        [ordered]@{
            name = 'export-cancellation-in-flight'
            arguments = @(
                'test',
                '-p',
                'myalbuns-desktop',
                '--lib',
                'export_pipeline::tests::cancellation_during_processing_discards_preparation_and_preserves_previous_output',
                '--',
                '--exact',
                '--nocapture'
            )
        },
        [ordered]@{
            name = 'processor-progress-stream'
            arguments = @(
                'test',
                '-p',
                'myalbuns-desktop',
                '--lib',
                'imaging_processor::tests::host_reports_chunked_progress_before_decoding_the_final_response',
                '--',
                '--exact',
                '--nocapture'
            )
        }
        }
    )
    foreach ($check in $rustChecks) {
        Invoke-RustCheck `
            -Name $check.name `
            -Arguments @($check.arguments)
    }

    Set-OperationProbeEnvironmentValue `
        -Name 'CARGO_TARGET_DIR' `
        -Value $targetDirectory

    $buildStartedAt = [DateTimeOffset]::UtcNow
    $buildStopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    & (Join-Path $PSScriptRoot 'Invoke-LocalTauri.ps1') build --no-bundle
    $buildExitCode = $LASTEXITCODE
    $buildStopwatch.Stop()
    if ($buildExitCode -ne 0) {
        throw "The real desktop build failed with exit code $buildExitCode."
    }
    if (-not (Test-Path -LiteralPath $executablePath -PathType Leaf)) {
        throw "The real desktop executable was not produced at '$executablePath'."
    }
    if (-not (Test-Path -LiteralPath $imagingExecutablePath -PathType Leaf)) {
        throw "The real imaging sidecar was not produced at '$imagingExecutablePath'."
    }
    $checks.Add([ordered]@{
        name = 'real-desktop-build'
        passed = $true
        elapsedMs = [long] $buildStopwatch.ElapsedMilliseconds
    })

    $buildInputState = Get-BuildInputState
    if (
        $buildInputState.fileCount -ne $initialBuildInputState.fileCount -or
        $buildInputState.digestSha256 -ne
            $initialBuildInputState.digestSha256 -or
        $buildInputState.dirty -ne $initialBuildInputState.dirty
    ) {
        throw (
            'OperationGate source inputs changed during tests or build; ' +
            'the executable cannot be tied to one source state.'
        )
    }
    $workingTreeStatus = @(
        & git -C $script:WorkspaceRoot status --short
    )
    if ($LASTEXITCODE -ne 0) {
        throw 'Could not inspect the OperationGate working tree.'
    }
    $gitCommit = (& git -C $script:WorkspaceRoot rev-parse HEAD).Trim()
    if ($LASTEXITCODE -ne 0 -or $gitCommit -notmatch '^[0-9a-f]{40}$') {
        throw 'Could not identify the OperationGate source commit.'
    }
    $build = [ordered]@{
        builtAtUtc = $buildStartedAt.ToString('o')
        elapsedMs = [long] $buildStopwatch.ElapsedMilliseconds
        profile = 'release'
        cargoBuildJobs = $CargoBuildJobs
        rustCheckTarget = 'target'
        executable = $executableRelativePath
        executableBytes = [long] (
            Get-Item -LiteralPath $executablePath
        ).Length
        executableSha256 = (
            Get-FileHash -LiteralPath $executablePath -Algorithm SHA256
        ).Hash.ToLowerInvariant()
        imagingExecutable = $imagingExecutableRelativePath
        imagingExecutableBytes = [long] (
            Get-Item -LiteralPath $imagingExecutablePath
        ).Length
        imagingExecutableSha256 = (
            Get-FileHash -LiteralPath $imagingExecutablePath -Algorithm SHA256
        ).Hash.ToLowerInvariant()
        buildInputFileCount = $buildInputState.fileCount
        buildInputDigestSha256 = $buildInputState.digestSha256
        buildInputsDirty = $buildInputState.dirty
        workingTreeDirty = $workingTreeStatus.Count -gt 0
    }

    if ($Suite -eq 'batch') {
        $batchResults = [ordered]@{}
        foreach ($scenario in $batchScenarios) {
            $result = Invoke-BatchLeaseScenario `
                -Scenario $scenario `
                -ProbeRoot $batchRoots[$scenario]
            $batchResults[$scenario] = $result
            $checks.Add([ordered]@{
                name = "batch-lease-$scenario"
                passed = $result.passed
                elapsedMs = $result.elapsedMs
            })
        }
        $finalBuildInputState = Get-BuildInputState
        if (
            $finalBuildInputState.fileCount -ne $buildInputState.fileCount -or
            $finalBuildInputState.digestSha256 -cne
                $buildInputState.digestSha256 -or
            $finalBuildInputState.dirty -ne $buildInputState.dirty
        ) {
            throw (
                'Batch lease probes changed source inputs after the build; ' +
                'the evidence cannot be tied to one source state.'
            )
        }
        $report = [ordered]@{
            schemaVersion = 1
            suite = 'batch_operation_lease'
            collectedAtUtc = [DateTimeOffset]::UtcNow.ToString('o')
            gitCommit = $gitCommit
            sourceInputsDirty = $finalBuildInputState.dirty
            platform = [ordered]@{
                operatingSystem = [System.Environment]::OSVersion.VersionString
                architecture = (
                    [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
                )
                powerShellEdition = [string] $PSVersionTable.PSEdition
                powerShellVersion = [string] $PSVersionTable.PSVersion
            }
            build = $build
            checks = @($checks)
            results = [ordered]@{
                topology = 'independent_two_host'
                scenarios = $batchResults
            }
            limits = [ordered]@{
                batchRunnerExecution = $true
                batchWideOperationLease = $true
                multiOutputPromotion = $true
                topologyMatrix = $false
                automaticDiscovery = $false
                checkpoint = $false
                resume = $false
                batchUi = $false
                orphanCleanup = $false
                projectOpenGuardian = $false
            }
        }
    }
    elseif ($Suite -eq 'project_open') {
        $report = Invoke-ProjectOpenGateSuite `
            -Context $projectOpenContext `
            -Build $build `
            -GitCommit $gitCommit `
            -BuildInputState $buildInputState `
            -Checks $checks
    }
    else {
    $independent = Invoke-OperationTopologyProbe `
        -Topology 'independent' `
        -ProbeRoot $independentRoot
    $multiwindow = Invoke-OperationTopologyProbe `
        -Topology 'multiwindow' `
        -ProbeRoot $multiwindowRoot
    $checks.Add([ordered]@{
        name = 'independent-two-host-operation-gate'
        passed = $independent.passed
        elapsedMs = $independent.elapsedMs
    })
    $checks.Add([ordered]@{
        name = 'multiwindow-two-window-operation-gate'
        passed = $multiwindow.passed
        elapsedMs = $multiwindow.elapsedMs
    })

    $terminalResults = [ordered]@{
        independent = [ordered]@{}
        multiwindow = [ordered]@{}
    }
    foreach ($topology in @('independent', 'multiwindow')) {
        foreach ($scenario in $terminalScenarios) {
            $result = Invoke-ExportTerminalScenario `
                -Topology $topology `
                -Scenario $scenario `
                -ProbeRoot $terminalRoots[$topology][$scenario]
            $terminalResults[$topology][$scenario] = $result
            $checks.Add([ordered]@{
                name = "export-terminal-$topology-$scenario"
                passed = $result.passed
                elapsedMs = $result.elapsedMs
            })
        }
    }
    $restoredImagingSha256 = (
        Get-FileHash -LiteralPath $imagingExecutablePath -Algorithm SHA256
    ).Hash.ToLowerInvariant()
    if ($restoredImagingSha256 -cne $build.imagingExecutableSha256) {
        throw 'The imaging sidecar was not restored byte-for-byte after fault injection.'
    }
    $checks.Add([ordered]@{
        name = 'imaging-sidecar-restored-after-failure-probes'
        passed = $true
        elapsedMs = 0
    })
    $finalBuildInputState = Get-BuildInputState
    if (
        $finalBuildInputState.fileCount -ne $buildInputState.fileCount -or
        $finalBuildInputState.digestSha256 -cne
            $buildInputState.digestSha256 -or
        $finalBuildInputState.dirty -ne $buildInputState.dirty
    ) {
        throw (
            'Export terminal probes changed source inputs after the build; ' +
            'the evidence cannot be tied to one source state.'
        )
    }

    $report = [ordered]@{
        schemaVersion = 2
        collectedAtUtc = [DateTimeOffset]::UtcNow.ToString('o')
        gitCommit = $gitCommit
        sourceInputsDirty = $finalBuildInputState.dirty
        platform = [ordered]@{
            operatingSystem = [System.Environment]::OSVersion.VersionString
            architecture = (
                [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
            )
            powerShellEdition = [string] $PSVersionTable.PSEdition
            powerShellVersion = [string] $PSVersionTable.PSVersion
        }
        build = $build
        checks = @($checks)
        results = [ordered]@{
            independent = $independent
            multiwindow = $multiwindow
            exportTerminalMatrix = $terminalResults
        }
        limits = [ordered]@{
            batchRunner = $false
            multiOutputPromotion = $false
            projectOpenGuardian = $false
            exportCancellationEntryPoint = $true
            progressChannel = $true
            uiCancellationFlow = $false
            progressWindow = $false
        }
    }
    }
}
finally {
    $cleanupErrors = [System.Collections.Generic.List[string]]::new()
    foreach ($process in @($startedProcesses)) {
        try {
            Stop-OwnedOperationProbeProcess -Process $process
        }
        catch {
            $cleanupErrors.Add($_.Exception.Message)
        }
    }
    if ($locationWasPushed) {
        try {
            Pop-Location
        }
        catch {
            $cleanupErrors.Add($_.Exception.Message)
        }
    }
    try {
        Set-OperationProbeEnvironmentValue `
            -Name 'CARGO_TARGET_DIR' `
            -Value $previousCargoTarget
    }
    catch {
        $cleanupErrors.Add($_.Exception.Message)
    }
    try {
        Set-OperationProbeEnvironmentValue `
            -Name 'CARGO_BUILD_JOBS' `
            -Value $previousCargoBuildJobs
    }
    catch {
        $cleanupErrors.Add($_.Exception.Message)
    }

    foreach ($topology in @('independent', 'multiwindow')) {
        $backupPath = Join-Path `
            $terminalRoots[$topology]['failure'] `
            'myalbuns-imaging.exe.disabled'
        if (Test-Path -LiteralPath $backupPath -PathType Leaf) {
            try {
                if (Test-Path -LiteralPath $imagingExecutablePath) {
                    throw (
                        'An imaging sidecar backup remains, but the release ' +
                        'destination is already occupied.'
                    )
                }
                Move-Item `
                    -LiteralPath $backupPath `
                    -Destination $imagingExecutablePath
            }
            catch {
                $cleanupErrors.Add($_.Exception.Message)
            }
        }
    }

    if ($cleanupErrors.Count -eq 0) {
        foreach ($fixtureRoot in $fixtureRoots) {
            try {
                if (Test-Path -LiteralPath $fixtureRoot) {
                    $verifiedRoot = [System.IO.Path]::GetFullPath($fixtureRoot)
                    if (-not $verifiedRoot.StartsWith(
                            $probeParent + [System.IO.Path]::DirectorySeparatorChar,
                            [System.StringComparison]::OrdinalIgnoreCase
                        )) {
                        throw 'Refusing to remove an unverified OperationGate probe root.'
                    }
                    Remove-Item -LiteralPath $verifiedRoot -Recurse -Force
                }
            }
            catch {
                $cleanupErrors.Add($_.Exception.Message)
            }
        }
    }
    if ($cleanupErrors.Count -gt 0) {
        throw (
            'OperationGate cleanup failed: ' +
            ($cleanupErrors -join ' | ')
        )
    }
}

if ($null -eq $report) {
    throw 'The OperationGate gate did not produce a report.'
}

$outputDirectory = Split-Path -Parent $OutputPath
New-Item -ItemType Directory -Force -Path $outputDirectory | Out-Null
$json = $report | ConvertTo-Json -Depth 10
[System.IO.File]::WriteAllText(
    $OutputPath,
    $json + [System.Environment]::NewLine,
    [System.Text.UTF8Encoding]::new($false)
)
Write-Output "OperationGate report: $OutputPath"
Write-Output $json
}
finally {
    if ($runnerMutexHeld) {
        try {
            $runnerMutex.ReleaseMutex()
            $runnerMutexHeld = $false
        }
        finally {
            $runnerMutex.Dispose()
        }
    }
    else {
        $runnerMutex.Dispose()
    }
}
