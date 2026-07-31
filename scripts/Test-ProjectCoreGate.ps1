param(
    [string] $OutputPath,
    [ValidateRange(10, 300)]
    [int] $ProbeTimeoutSeconds = 90,
    [ValidateRange(1, 8)]
    [int] $CargoBuildJobs = 1
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

. (Join-Path $PSScriptRoot 'Local-Toolchain.ps1')
Initialize-MyAlbunsToolchain
. (Join-Path $PSScriptRoot 'Evidence-BuildInputs.ps1')

if ($env:OS -ne 'Windows_NT') {
    throw 'The ProjectCore gate must run on Windows.'
}

if ([string]::IsNullOrWhiteSpace($OutputPath)) {
    $OutputPath = Join-Path `
        $script:WorkspaceRoot `
        'docs\research\artifacts\0013-project-core-session-revision.json'
}
elseif (-not [System.IO.Path]::IsPathRooted($OutputPath)) {
    $OutputPath = Join-Path $script:WorkspaceRoot $OutputPath
}
$OutputPath = [System.IO.Path]::GetFullPath($OutputPath)

$probeParent = [System.IO.Path]::GetFullPath(
    (Join-Path $script:WorkspaceRoot '.scratch\project-core-probe')
)
$runId = "$PID-$([DateTime]::UtcNow.Ticks)-$([Guid]::NewGuid().ToString('N'))"
$probeRoot = [System.IO.Path]::GetFullPath(
    (Join-Path $probeParent "run-$runId")
)
if (-not $probeRoot.StartsWith(
        $probeParent + [System.IO.Path]::DirectorySeparatorChar,
        [System.StringComparison]::OrdinalIgnoreCase
    )) {
    throw 'The ProjectCore probe root escaped the workspace scratch directory.'
}
if (Test-Path -LiteralPath $probeRoot) {
    throw 'The unique ProjectCore probe root already exists.'
}
New-Item -ItemType Directory -Path $probeRoot | Out-Null

$targetDirectory = [System.IO.Path]::GetFullPath(
    (Join-Path $script:WorkspaceRoot 'target\project-core-gate')
)
$rustCheckTargetDirectory = [System.IO.Path]::GetFullPath(
    (Join-Path $script:WorkspaceRoot 'target')
)
$executablePath = Join-Path `
    $targetDirectory `
    'release\myalbuns-desktop.exe'
$executableRelativePath =
    'target/project-core-gate/release/myalbuns-desktop.exe'
$preparedPath = Join-Path $probeRoot 'prepared.json'
$readyPath = Join-Path $probeRoot 'ready.json'
$completedPath = Join-Path $probeRoot 'completed.json'
$continuePath = Join-Path $probeRoot 'continue.signal'
$inputPaths = @(
    (Join-Path $probeRoot 'inputs\Horizon.myalbum'),
    (Join-Path $probeRoot 'inputs\Aurora.myalbum')
)
$outputRoot = Join-Path $probeRoot 'outputs'

$checks = [System.Collections.Generic.List[object]]::new()
$startedProcesses =
    [System.Collections.Generic.List[System.Diagnostics.Process]]::new()
$inputStreams = [System.Collections.Generic.List[System.IO.FileStream]]::new()
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

function Set-ProjectCoreRunnerEnvironmentValue {
    param(
        [Parameter(Mandatory = $true)][string] $Name,
        [AllowNull()][string] $Value
    )

    [System.Environment]::SetEnvironmentVariable(
        $Name,
        $Value,
        [System.EnvironmentVariableTarget]::Process
    )
}

function Assert-ProjectCoreRunnerAst {
    $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    $tokens = $null
    $errors = $null
    [void] [System.Management.Automation.Language.Parser]::ParseFile(
        $PSCommandPath,
        [ref] $tokens,
        [ref] $errors
    )
    $stopwatch.Stop()
    if (@($errors).Count -gt 0) {
        $messages = @($errors | ForEach-Object { $_.Message }) -join ' | '
        throw "The ProjectCore runner has PowerShell parser errors: $messages"
    }
    $checks.Add([ordered]@{
        name = 'powershell-runner-ast'
        passed = $true
        elapsedMs = [long] $stopwatch.ElapsedMilliseconds
    })
}

function Assert-NoDuplicateProjectCoreJsonNames {
    param([Parameter(Mandatory = $true)][string] $Json)

    # ConvertFrom-Json accepts duplicate names. Track an ordinal name set for
    # every object before deserializing the closed probe contracts.
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
                throw 'The ProjectCore event contains unbalanced JSON scopes.'
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
            throw 'The ProjectCore event contains an unterminated JSON string.'
        }
        $lookahead = $index + 1
        while (
            $lookahead -lt $Json.Length -and
            [char]::IsWhiteSpace($Json[$lookahead])
        ) {
            $lookahead++
        }
        if ($lookahead -ge $Json.Length -or $Json[$lookahead] -ne ':') {
            continue
        }
        if (
            $scopes.Count -eq 0 -or
            $scopes[$scopes.Count - 1] -isnot
                [System.Collections.Generic.HashSet[string]]
        ) {
            throw 'The ProjectCore event contains a property outside an object.'
        }
        $rawName = $Json.Substring($tokenStart, $index - $tokenStart + 1)
        $name = $rawName | ConvertFrom-Json
        $seenNames = $scopes[$scopes.Count - 1]
        if (-not $seenNames.Add([string] $name)) {
            throw "The ProjectCore event repeats the JSON field '$name'."
        }
    }
    if ($scopes.Count -ne 0) {
        throw 'The ProjectCore event contains unbalanced JSON scopes.'
    }
}

function Assert-ExactProjectCoreProperties {
    param(
        [Parameter(Mandatory = $true)][object] $Value,
        [Parameter(Mandatory = $true)][string[]] $ExpectedNames,
        [Parameter(Mandatory = $true)][string] $Label
    )

    if ($Value -isnot [System.Management.Automation.PSCustomObject]) {
        throw "$Label must be a JSON object."
    }
    $actualNames = @($Value.PSObject.Properties.Name)
    $missing = @(
        $ExpectedNames | Where-Object { $actualNames -cnotcontains $_ }
    )
    $unexpected = @(
        $actualNames | Where-Object { $ExpectedNames -cnotcontains $_ }
    )
    if ($missing.Count -gt 0 -or $unexpected.Count -gt 0) {
        throw (
            "$Label fields differ from the closed schema. " +
            "Missing: $($missing -join ', '); " +
            "unexpected: $($unexpected -join ', ')."
        )
    }
}

function Read-StrictProjectCoreJson {
    param(
        [Parameter(Mandatory = $true)][string] $Path,
        [Parameter(Mandatory = $true)][string[]] $ExpectedNames,
        [Parameter(Mandatory = $true)][string] $Label
    )

    $json = [System.IO.File]::ReadAllText(
        $Path,
        [System.Text.Encoding]::UTF8
    )
    Assert-NoDuplicateProjectCoreJsonNames -Json $json
    try {
        $root = $json | ConvertFrom-Json
    }
    catch {
        throw "$Label is not valid JSON: $($_.Exception.Message)"
    }
    Assert-ExactProjectCoreProperties `
        -Value $root `
        -ExpectedNames $ExpectedNames `
        -Label $Label
    return $root
}

function ConvertTo-ProjectCoreInteger {
    param(
        [Parameter(Mandatory = $true)][object] $Value,
        [Parameter(Mandatory = $true)][string] $Label,
        [switch] $AllowZero
    )

    if ($Value -isnot [int] -and $Value -isnot [long]) {
        throw "$Label must be an integer."
    }
    $integer = [long] $Value
    if (($AllowZero -and $integer -lt 0) -or (-not $AllowZero -and $integer -lt 1)) {
        throw "$Label is outside its allowed range."
    }
    return $integer
}

function Assert-ProjectCoreString {
    param(
        [Parameter(Mandatory = $true)][object] $Value,
        [Parameter(Mandatory = $true)][string] $Label,
        [string] $Expected
    )

    if ($Value -isnot [string] -or [string]::IsNullOrWhiteSpace($Value)) {
        throw "$Label must be a non-empty string."
    }
    if ($PSBoundParameters.ContainsKey('Expected') -and $Value -cne $Expected) {
        throw "$Label must be '$Expected', received '$Value'."
    }
    return [string] $Value
}

function Assert-ProjectCoreBoolean {
    param(
        [Parameter(Mandatory = $true)][object] $Value,
        [Parameter(Mandatory = $true)][string] $Label,
        [Parameter(Mandatory = $true)][bool] $Expected
    )

    if ($Value -isnot [bool] -or [bool] $Value -ne $Expected) {
        throw "$Label must be the boolean '$Expected'."
    }
}

function Read-PreparedProjectCoreEvent {
    param([Parameter(Mandatory = $true)][string] $Path)

    $root = Read-StrictProjectCoreJson `
        -Path $Path `
        -Label 'The ProjectCore prepared event' `
        -ExpectedNames @('schemaVersion', 'processId', 'state', 'inputCount')
    $schemaVersion = ConvertTo-ProjectCoreInteger `
        -Value $root.schemaVersion `
        -Label 'prepared.schemaVersion'
    $processId = ConvertTo-ProjectCoreInteger `
        -Value $root.processId `
        -Label 'prepared.processId'
    $inputCount = ConvertTo-ProjectCoreInteger `
        -Value $root.inputCount `
        -Label 'prepared.inputCount'
    [void] (Assert-ProjectCoreString `
        -Value $root.state `
        -Label 'prepared.state' `
        -Expected 'prepared')
    if ($schemaVersion -ne 1 -or $inputCount -ne 2) {
        throw 'The ProjectCore prepared event has invalid scalar values.'
    }
    return [ordered]@{
        schemaVersion = [int] $schemaVersion
        processId = [int] $processId
        state = 'prepared'
        inputCount = [int] $inputCount
    }
}

function Read-ReadyProjectCoreEvent {
    param([Parameter(Mandatory = $true)][string] $Path)

    $names = @(
        'schemaVersion',
        'processId',
        'state',
        'runMode',
        'processRole',
        'projectHostConstructed',
        'editableProjectOwned'
    )
    $root = Read-StrictProjectCoreJson `
        -Path $Path `
        -Label 'The ProjectCore ready event' `
        -ExpectedNames $names
    $schemaVersion = ConvertTo-ProjectCoreInteger `
        -Value $root.schemaVersion `
        -Label 'ready.schemaVersion'
    $processId = ConvertTo-ProjectCoreInteger `
        -Value $root.processId `
        -Label 'ready.processId'
    [void] (Assert-ProjectCoreString `
        -Value $root.state `
        -Label 'ready.state' `
        -Expected 'ready')
    [void] (Assert-ProjectCoreString `
        -Value $root.runMode `
        -Label 'ready.runMode' `
        -Expected 'headless_before_project_host')
    [void] (Assert-ProjectCoreString `
        -Value $root.processRole `
        -Label 'ready.processRole' `
        -Expected 'global')
    Assert-ProjectCoreBoolean `
        -Value $root.projectHostConstructed `
        -Label 'ready.projectHostConstructed' `
        -Expected $false
    Assert-ProjectCoreBoolean `
        -Value $root.editableProjectOwned `
        -Label 'ready.editableProjectOwned' `
        -Expected $false
    if ($schemaVersion -ne 1) {
        throw 'The ProjectCore ready event has an unsupported schema.'
    }
    return [ordered]@{
        schemaVersion = [int] $schemaVersion
        processId = [int] $processId
        state = 'ready'
        runMode = 'headless_before_project_host'
        processRole = 'global'
        projectHostConstructed = $false
        editableProjectOwned = $false
    }
}

function Read-CompletedProjectCoreEvent {
    param([Parameter(Mandatory = $true)][string] $Path)

    $names = @(
        'schemaVersion',
        'processId',
        'state',
        'runMode',
        'processRole',
        'projectHostConstructed',
        'editableProjectOwned',
        'inputType',
        'loadedRevisionCount',
        'completedItemCount',
        'publishedOutputCount',
        'batchCompletedEventCount',
        'items',
        'renders'
    )
    $root = Read-StrictProjectCoreJson `
        -Path $Path `
        -Label 'The ProjectCore completed event' `
        -ExpectedNames $names

    $integerFields = @{}
    foreach ($name in @(
            'schemaVersion',
            'processId',
            'loadedRevisionCount',
            'completedItemCount',
            'publishedOutputCount',
            'batchCompletedEventCount'
        )) {
        $integerFields[$name] = ConvertTo-ProjectCoreInteger `
            -Value $root.$name `
            -Label "completed.$name"
    }
    [void] (Assert-ProjectCoreString `
        -Value $root.state `
        -Label 'completed.state' `
        -Expected 'completed')
    [void] (Assert-ProjectCoreString `
        -Value $root.runMode `
        -Label 'completed.runMode' `
        -Expected 'headless_before_project_host')
    [void] (Assert-ProjectCoreString `
        -Value $root.processRole `
        -Label 'completed.processRole' `
        -Expected 'global')
    [void] (Assert-ProjectCoreString `
        -Value $root.inputType `
        -Label 'completed.inputType' `
        -Expected 'loaded_project_revision')
    Assert-ProjectCoreBoolean `
        -Value $root.projectHostConstructed `
        -Label 'completed.projectHostConstructed' `
        -Expected $false
    Assert-ProjectCoreBoolean `
        -Value $root.editableProjectOwned `
        -Label 'completed.editableProjectOwned' `
        -Expected $false
    if (
        $integerFields.schemaVersion -ne 1 -or
        $integerFields.loadedRevisionCount -ne 2 -or
        $integerFields.completedItemCount -ne 2 -or
        $integerFields.publishedOutputCount -ne 2 -or
        $integerFields.batchCompletedEventCount -ne 1
    ) {
        throw 'The ProjectCore completed event has invalid scalar counts.'
    }

    $items = @($root.items)
    if ($items.Count -ne 2) {
        throw 'The ProjectCore completed event must contain exactly two items.'
    }
    $normalizedItems = @(
        for ($index = 0; $index -lt $items.Count; $index++) {
            $item = $items[$index]
            Assert-ExactProjectCoreProperties `
                -Value $item `
                -ExpectedNames @('itemId', 'projectId', 'revision') `
                -Label "completed.items[$index]"
            [ordered]@{
                itemId = Assert-ProjectCoreString `
                    -Value $item.itemId `
                    -Label "completed.items[$index].itemId"
                projectId = Assert-ProjectCoreString `
                    -Value $item.projectId `
                    -Label "completed.items[$index].projectId"
                revision = ConvertTo-ProjectCoreInteger `
                    -Value $item.revision `
                    -Label "completed.items[$index].revision" `
                    -AllowZero
            }
        }
    )
    if (@($normalizedItems.itemId | Sort-Object -Unique).Count -ne 2) {
        throw 'The ProjectCore completed item identifiers must be unique.'
    }

    $renders = @($root.renders)
    if ($renders.Count -ne 2) {
        throw 'The ProjectCore completed event must contain exactly two renders.'
    }
    $normalizedRenders = @(
        for ($index = 0; $index -lt $renders.Count; $index++) {
            $render = $renders[$index]
            Assert-ExactProjectCoreProperties `
                -Value $render `
                -ExpectedNames @(
                    'requestId',
                    'projectId',
                    'revision',
                    'outputBytes',
                    'outputSha256'
                ) `
                -Label "completed.renders[$index]"
            $sha256 = Assert-ProjectCoreString `
                -Value $render.outputSha256 `
                -Label "completed.renders[$index].outputSha256"
            if ($sha256 -cnotmatch '^[0-9a-f]{64}$') {
                throw "completed.renders[$index].outputSha256 is not canonical SHA-256."
            }
            [ordered]@{
                requestId = Assert-ProjectCoreString `
                    -Value $render.requestId `
                    -Label "completed.renders[$index].requestId"
                projectId = Assert-ProjectCoreString `
                    -Value $render.projectId `
                    -Label "completed.renders[$index].projectId"
                revision = ConvertTo-ProjectCoreInteger `
                    -Value $render.revision `
                    -Label "completed.renders[$index].revision" `
                    -AllowZero
                outputBytes = ConvertTo-ProjectCoreInteger `
                    -Value $render.outputBytes `
                    -Label "completed.renders[$index].outputBytes"
                outputSha256 = $sha256
            }
        }
    )
    if (@($normalizedRenders.requestId | Sort-Object -Unique).Count -ne 2) {
        throw 'The ProjectCore render request identifiers must be unique.'
    }

    return [ordered]@{
        schemaVersion = 1
        processId = [int] $integerFields.processId
        state = 'completed'
        runMode = 'headless_before_project_host'
        processRole = 'global'
        projectHostConstructed = $false
        editableProjectOwned = $false
        inputType = 'loaded_project_revision'
        loadedRevisionCount = 2
        completedItemCount = 2
        publishedOutputCount = 2
        batchCompletedEventCount = 1
        items = @($normalizedItems)
        renders = @($normalizedRenders)
    }
}

function Test-StrictProjectCoreParsers {
    $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    $parserRoot = Join-Path $probeRoot 'parser-contract'
    New-Item -ItemType Directory -Path $parserRoot | Out-Null
    $path = Join-Path $parserRoot 'event.json'
    $valid = @'
{"schemaVersion":1,"processId":42,"state":"ready","runMode":"headless_before_project_host","processRole":"global","projectHostConstructed":false,"editableProjectOwned":false}
'@
    [System.IO.File]::WriteAllText(
        $path,
        $valid.Trim(),
        [System.Text.UTF8Encoding]::new($false)
    )
    $event = Read-ReadyProjectCoreEvent -Path $path
    if ($event.processId -ne 42 -or $event.state -cne 'ready') {
        throw 'The ProjectCore parser changed a valid ready event.'
    }

    $invalidCases = @(
        [ordered]@{
            label = 'duplicate field'
            json = $valid.Replace(
                '"schemaVersion":1',
                '"schemaVersion":1,"schema\u0056ersion":2'
            )
            error = '^The ProjectCore event repeats the JSON field'
        },
        [ordered]@{
            label = 'unexpected field'
            json = $valid.Replace(
                '"editableProjectOwned":false}',
                '"editableProjectOwned":false,"unexpected":true}'
            )
            error = '^The ProjectCore ready event fields differ from the closed schema'
        },
        [ordered]@{
            label = 'mis-cased field'
            json = $valid.Replace('"runMode":', '"RunMode":')
            error = '^The ProjectCore ready event fields differ from the closed schema'
        }
    )
    foreach ($invalidCase in $invalidCases) {
        [System.IO.File]::WriteAllText(
            $path,
            [string] $invalidCase.json,
            [System.Text.UTF8Encoding]::new($false)
        )
        try {
            [void] (Read-ReadyProjectCoreEvent -Path $path)
            throw "The ProjectCore parser accepted $($invalidCase.label)."
        }
        catch {
            if ($_.Exception.Message -cnotmatch $invalidCase.error) {
                throw
            }
        }
    }
    $stopwatch.Stop()
    $checks.Add([ordered]@{
        name = 'strict-project-core-json-parsers'
        passed = $true
        elapsedMs = [long] $stopwatch.ElapsedMilliseconds
    })
}

function Invoke-ProjectCoreRustCheck {
    param(
        [Parameter(Mandatory = $true)][string] $Name,
        [Parameter(Mandatory = $true)][string[]] $Arguments
    )

    $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    $previousPreference = $ErrorActionPreference
    try {
        $ErrorActionPreference = 'Continue'
        $commandOutput = @(& $script:CargoExecutable @Arguments 2>&1)
        $exitCode = $LASTEXITCODE
    }
    finally {
        $ErrorActionPreference = $previousPreference
    }
    $stopwatch.Stop()
    foreach ($line in $commandOutput) {
        Write-Host $line
    }
    if ($exitCode -ne 0) {
        throw "ProjectCore check '$Name' failed with exit code $exitCode."
    }
    $transcript = $commandOutput -join [System.Environment]::NewLine
    if ($transcript -notmatch '(?m)^running [1-9][0-9]* tests?') {
        throw "ProjectCore check '$Name' did not execute any test."
    }
    if (
        $transcript -notmatch
            '(?m)^test result: ok\. [1-9][0-9]* passed; 0 failed;'
    ) {
        throw "ProjectCore check '$Name' did not pass a non-ignored test."
    }
    $checks.Add([ordered]@{
        name = $Name
        passed = $true
        elapsedMs = [long] $stopwatch.ElapsedMilliseconds
    })
}

function Start-ProjectCoreProbeProcess {
    param(
        [Parameter(Mandatory = $true)]
        [ValidateSet('prepare', 'batch')]
        [string] $Action
    )

    $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $executablePath
    $startInfo.WorkingDirectory = $script:WorkspaceRoot
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    foreach ($name in @(
            'MYALBUNS_TOPOLOGY_SPIKE',
            'MYALBUNS_GLOBAL_SPIKE_ENDPOINT',
            'MYALBUNS_OPERATION_GATE_PROBE_ROOT',
            'MYALBUNS_EXPORT_TERMINAL_PROBE_ROOT',
            'MYALBUNS_BATCH_LEASE_PROBE_ROOT',
            'MYALBUNS_PROJECT_OPEN_PROBE_ROOT'
        )) {
        [void] $startInfo.EnvironmentVariables.Remove($name)
    }
    $startInfo.EnvironmentVariables['MYALBUNS_PROCESS_ROLE'] = 'global'
    $startInfo.EnvironmentVariables['MYALBUNS_PROJECT_CORE_PROBE_ROOT'] =
        $probeRoot
    $startInfo.EnvironmentVariables['MYALBUNS_PROJECT_CORE_PROBE_ACTION'] =
        $Action

    $process = [System.Diagnostics.Process]::new()
    $process.StartInfo = $startInfo
    try {
        if (-not $process.Start()) {
            throw "Could not start the ProjectCore '$Action' probe."
        }
        $startedProcesses.Add($process)
        return $process
    }
    catch {
        $process.Dispose()
        throw
    }
}

function Wait-ProjectCoreProcessExit {
    param(
        [Parameter(Mandatory = $true)]
        [System.Diagnostics.Process] $Process,
        [Parameter(Mandatory = $true)][string] $Label
    )

    if (-not $Process.WaitForExit($ProbeTimeoutSeconds * 1000)) {
        throw "The ProjectCore $Label process timed out."
    }
    $exitCode = $Process.ExitCode
    if ($exitCode -ne 0) {
        throw "The ProjectCore $Label process exited with code $exitCode."
    }
}

function Wait-ProjectCoreProbeFile {
    param(
        [Parameter(Mandatory = $true)][string] $Path,
        [Parameter(Mandatory = $true)]
        [System.Diagnostics.Process] $Process,
        [Parameter(Mandatory = $true)][string] $Label
    )

    $stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
    while ($stopwatch.Elapsed.TotalSeconds -lt $ProbeTimeoutSeconds) {
        if (
            (Test-Path -LiteralPath $Path -PathType Leaf) -and
            (Get-Item -LiteralPath $Path).Length -gt 0
        ) {
            Start-Sleep -Milliseconds 25
            return
        }
        if ($Process.HasExited) {
            $Process.WaitForExit()
            throw (
                "The ProjectCore $Label process exited with code " +
                "$($Process.ExitCode) before producing '$Path'."
            )
        }
        Start-Sleep -Milliseconds 25
    }
    throw "Timed out waiting for the ProjectCore $Label file '$Path'."
}

function Stop-OwnedProjectCoreProcess {
    param(
        [Parameter(Mandatory = $true)]
        [System.Diagnostics.Process] $Process
    )

    $stopped = $false
    try {
        if (-not $Process.HasExited) {
            $Process.Kill()
            if (-not $Process.WaitForExit(10000)) {
                throw "ProjectCore process $($Process.Id) did not terminate."
            }
        }
        $stopped = $true
    }
    finally {
        if ($stopped) {
            [void] $startedProcesses.Remove($Process)
            $Process.Dispose()
        }
    }
}

function Get-ProjectCoreInputEvidence {
    param([Parameter(Mandatory = $true)][string] $Path)

    $item = Get-Item -LiteralPath $Path
    if (
        $item.PSIsContainer -or
        ($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0 -or
        $item.Length -lt 1
    ) {
        throw "ProjectCore input '$Path' is not a non-empty regular file."
    }
    $document = Get-Content -LiteralPath $Path -Raw -Encoding utf8 |
        ConvertFrom-Json
    $projectId = Assert-ProjectCoreString `
        -Value $document.projectId `
        -Label "$($item.Name).projectId"
    $revision = ConvertTo-ProjectCoreInteger `
        -Value $document.revision `
        -Label "$($item.Name).revision" `
        -AllowZero
    return [ordered]@{
        name = $item.Name
        bytes = [long] $item.Length
        sha256 = (
            Get-FileHash -LiteralPath $Path -Algorithm SHA256
        ).Hash.ToLowerInvariant()
        projectId = $projectId
        revision = [long] $revision
    }
}

function Assert-ProjectCoreInputsUnchanged {
    param(
        [Parameter(Mandatory = $true)][object[]] $Before,
        [Parameter(Mandatory = $true)][object[]] $After
    )

    if ($Before.Count -ne 2 -or $After.Count -ne 2) {
        throw 'The ProjectCore gate requires exactly two input measurements.'
    }
    for ($index = 0; $index -lt 2; $index++) {
        if (
            [string] $Before[$index].name -cne [string] $After[$index].name -or
            [long] $Before[$index].bytes -ne [long] $After[$index].bytes -or
            [string] $Before[$index].sha256 -cne [string] $After[$index].sha256 -or
            [string] $Before[$index].projectId -cne
                [string] $After[$index].projectId -or
            [long] $Before[$index].revision -ne
                [long] $After[$index].revision
        ) {
            throw "ProjectCore input '$($Before[$index].name)' changed during Batch."
        }
    }
}

function Test-ProjectCoreInputWriteAndDeleteBlocked {
    param([Parameter(Mandatory = $true)][string] $Path)

    $writeStream = $null
    $writeBlocked = $false
    try {
        $writeStream = [System.IO.File]::Open(
            $Path,
            [System.IO.FileMode]::Open,
            [System.IO.FileAccess]::Write,
            [System.IO.FileShare]::ReadWrite
        )
    }
    catch [System.IO.IOException] {
        $writeBlocked = $true
    }
    catch [System.UnauthorizedAccessException] {
        $writeBlocked = $true
    }
    finally {
        if ($null -ne $writeStream) {
            $writeStream.Dispose()
        }
    }
    if (-not $writeBlocked) {
        throw "A write handle opened for the read-locked Project input '$Path'."
    }

    $deleteBlocked = $false
    try {
        [System.IO.File]::Delete($Path)
    }
    catch [System.IO.IOException] {
        $deleteBlocked = $true
    }
    catch [System.UnauthorizedAccessException] {
        $deleteBlocked = $true
    }
    if (-not $deleteBlocked -or -not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "Deletion was not blocked for the read-locked Project input '$Path'."
    }

    return [ordered]@{
        name = [System.IO.Path]::GetFileName($Path)
        writeOpenBlocked = $true
        deleteBlocked = $true
    }
}

function Get-ProjectCoreOutputEvidence {
    if (-not (Test-Path -LiteralPath $outputRoot -PathType Container)) {
        throw 'The ProjectCore Batch did not create its output directory.'
    }
    $files = @(
        Get-ChildItem -LiteralPath $outputRoot -Recurse -File |
            Sort-Object FullName
    )
    if ($files.Count -ne 2) {
        throw "The ProjectCore Batch produced $($files.Count) output files, expected 2."
    }
    return @(
        foreach ($file in $files) {
            if (
                ($file.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0 -or
                $file.Length -lt 1
            ) {
                throw "ProjectCore output '$($file.FullName)' is not a non-empty regular file."
            }
            [ordered]@{
                relativePath = $file.FullName.Substring(
                    $probeRoot.Length + 1
                ).Replace('\', '/')
                bytes = [long] $file.Length
                sha256 = (
                    Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256
                ).Hash.ToLowerInvariant()
            }
        }
    )
}

function Assert-ProjectCoreBatchCorrelation {
    param(
        [Parameter(Mandatory = $true)][object[]] $Inputs,
        [Parameter(Mandatory = $true)][object] $Completed,
        [Parameter(Mandatory = $true)][object[]] $Outputs
    )

    $inputKeys = @(
        $Inputs |
            ForEach-Object { "$($_.projectId)`0$($_.revision)" } |
            Sort-Object
    )
    $itemKeys = @(
        $Completed.items |
            ForEach-Object { "$($_.projectId)`0$($_.revision)" } |
            Sort-Object
    )
    $renderKeys = @(
        $Completed.renders |
            ForEach-Object { "$($_.projectId)`0$($_.revision)" } |
            Sort-Object
    )
    if (
        ($inputKeys -join "`n") -cne ($itemKeys -join "`n") -or
        ($itemKeys -join "`n") -cne ($renderKeys -join "`n")
    ) {
        throw 'ProjectCore inputs, Batch items, and render requests do not identify the same persisted revisions.'
    }
    if (@($inputKeys | Sort-Object -Unique).Count -ne 2) {
        throw 'The ProjectCore fixtures do not represent two distinct persisted revisions.'
    }

    $renderOutputs = @(
        $Completed.renders |
            ForEach-Object { "$($_.outputBytes)`0$($_.outputSha256)" } |
            Sort-Object
    )
    $publishedOutputs = @(
        $Outputs |
            ForEach-Object { "$($_.bytes)`0$($_.sha256)" } |
            Sort-Object
    )
    if (($renderOutputs -join "`n") -cne ($publishedOutputs -join "`n")) {
        throw 'The ProjectCore render evidence does not match the two published outputs.'
    }
}

$runnerMutex = [System.Threading.Mutex]::new(
    $false,
    'Local\MyAlbuns.ProjectCoreEvidence.v1'
)
$runnerMutexHeld = $false
try {
    try {
        $runnerMutexHeld = $runnerMutex.WaitOne(0)
    }
    catch [System.Threading.AbandonedMutexException] {
        $runnerMutexHeld = $true
    }
    if (-not $runnerMutexHeld) {
        throw 'Another ProjectCore evidence runner is using the shared build target.'
    }

    try {
        Push-Location $script:WorkspaceRoot
        $locationWasPushed = $true
        Assert-ProjectCoreRunnerAst
        Test-StrictProjectCoreParsers

        $initialBuildInputState = Get-BuildInputState
        if ($initialBuildInputState.dirty) {
            throw (
                'ProjectCore evidence requires clean build inputs. ' +
                'Commit or revert changes under the evidence build pathspecs first.'
            )
        }

        Set-ProjectCoreRunnerEnvironmentValue `
            -Name 'CARGO_TARGET_DIR' `
            -Value $rustCheckTargetDirectory
        Set-ProjectCoreRunnerEnvironmentValue `
            -Name 'CARGO_BUILD_JOBS' `
            -Value ([string] $CargoBuildJobs)

        $rustChecks = @(
            [ordered]@{
                name = 'project-core-public-seams'
                arguments = @(
                    'test',
                    '-p',
                    'myalbuns-core',
                    '--lib',
                    '--',
                    '--nocapture'
                )
            },
            [ordered]@{
                name = 'project-host-single-editable-owner'
                arguments = @(
                    'test',
                    '-p',
                    'myalbuns-desktop',
                    '--lib',
                    'project_host::tests::',
                    '--',
                    '--nocapture'
                )
            },
            [ordered]@{
                name = 'batch-runner-persisted-revision-input'
                arguments = @(
                    'test',
                    '-p',
                    'myalbuns-desktop',
                    '--lib',
                    'batch_runner::tests::',
                    '--',
                    '--nocapture'
                )
            },
            [ordered]@{
                name = 'project-core-probe-contract'
                arguments = @(
                    'test',
                    '-p',
                    'myalbuns-desktop',
                    '--lib',
                    'project_core_probe::tests::',
                    '--',
                    '--nocapture'
                )
            }
        )
        foreach ($check in $rustChecks) {
            Invoke-ProjectCoreRustCheck `
                -Name $check.name `
                -Arguments @($check.arguments)
        }

        Set-ProjectCoreRunnerEnvironmentValue `
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
        $checks.Add([ordered]@{
            name = 'real-release-desktop-build'
            passed = $true
            elapsedMs = [long] $buildStopwatch.ElapsedMilliseconds
        })

        $buildInputState = Get-BuildInputState
        if (
            $buildInputState.fileCount -ne $initialBuildInputState.fileCount -or
            $buildInputState.digestSha256 -cne
                $initialBuildInputState.digestSha256 -or
            $buildInputState.dirty
        ) {
            throw (
                'ProjectCore source inputs changed during tests or build; ' +
                'the executable cannot be tied to one clean source state.'
            )
        }

        $prepareStopwatch = [System.Diagnostics.Stopwatch]::StartNew()
        $prepareProcess = Start-ProjectCoreProbeProcess -Action 'prepare'
        $prepareProcessId = $prepareProcess.Id
        Wait-ProjectCoreProcessExit `
            -Process $prepareProcess `
            -Label 'prepare'
        if (-not (Test-Path -LiteralPath $preparedPath -PathType Leaf)) {
            throw 'The ProjectCore prepare action did not create prepared.json.'
        }
        $prepared = Read-PreparedProjectCoreEvent -Path $preparedPath
        if ($prepared.processId -ne $prepareProcessId) {
            throw 'prepared.json does not belong to the process started by the runner.'
        }
        Stop-OwnedProjectCoreProcess -Process $prepareProcess
        $prepareStopwatch.Stop()
        $checks.Add([ordered]@{
            name = 'prepare-two-persisted-projects'
            passed = $true
            elapsedMs = [long] $prepareStopwatch.ElapsedMilliseconds
        })

        $inputsBefore = @(
            foreach ($path in $inputPaths) {
                if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
                    throw "The ProjectCore prepare action did not create '$path'."
                }
                Get-ProjectCoreInputEvidence -Path $path
            }
        )
        if (@($inputsBefore.projectId | Sort-Object -Unique).Count -ne 2) {
            throw 'The prepared ProjectCore inputs must have distinct Project identities.'
        }

        $batchStopwatch = [System.Diagnostics.Stopwatch]::StartNew()
        $batchProcess = Start-ProjectCoreProbeProcess -Action 'batch'
        $batchProcessId = $batchProcess.Id
        Wait-ProjectCoreProbeFile `
            -Path $readyPath `
            -Process $batchProcess `
            -Label 'batch ready'
        $ready = Read-ReadyProjectCoreEvent -Path $readyPath
        if ($ready.processId -ne $batchProcessId) {
            throw 'ready.json does not belong to the Batch process started by the runner.'
        }

        $lockChecks = @()
        foreach ($path in $inputPaths) {
            $stream = [System.IO.File]::Open(
                $path,
                [System.IO.FileMode]::Open,
                [System.IO.FileAccess]::Read,
                [System.IO.FileShare]::Read
            )
            $inputStreams.Add($stream)
        }
        foreach ($path in $inputPaths) {
            $lockChecks += Test-ProjectCoreInputWriteAndDeleteBlocked -Path $path
        }

        [System.IO.File]::WriteAllBytes(
            $continuePath,
            [byte[]] @(1)
        )
        Wait-ProjectCoreProbeFile `
            -Path $completedPath `
            -Process $batchProcess `
            -Label 'batch completion'
        $completed = Read-CompletedProjectCoreEvent -Path $completedPath
        if ($completed.processId -ne $batchProcessId) {
            throw 'completed.json does not belong to the Batch process started by the runner.'
        }
        Wait-ProjectCoreProcessExit `
            -Process $batchProcess `
            -Label 'batch'
        $inputsAfter = @(
            foreach ($path in $inputPaths) {
                Get-ProjectCoreInputEvidence -Path $path
            }
        )
        Assert-ProjectCoreInputsUnchanged `
            -Before $inputsBefore `
            -After $inputsAfter
        $outputs = @(Get-ProjectCoreOutputEvidence)
        Assert-ProjectCoreBatchCorrelation `
            -Inputs $inputsBefore `
            -Completed $completed `
            -Outputs $outputs
        Stop-OwnedProjectCoreProcess -Process $batchProcess
        $batchStopwatch.Stop()
        $checks.Add([ordered]@{
            name = 'headless-read-only-two-item-batch'
            passed = $true
            elapsedMs = [long] $batchStopwatch.ElapsedMilliseconds
        })

        foreach ($stream in @($inputStreams)) {
            $stream.Dispose()
            [void] $inputStreams.Remove($stream)
        }

        $finalBuildInputState = Get-BuildInputState
        if (
            $finalBuildInputState.fileCount -ne $buildInputState.fileCount -or
            $finalBuildInputState.digestSha256 -cne
                $buildInputState.digestSha256 -or
            $finalBuildInputState.dirty
        ) {
            throw (
                'ProjectCore probes changed source inputs after the build; ' +
                'the evidence cannot be tied to one clean source state.'
            )
        }
        $gitCommit = (& git -C $script:WorkspaceRoot rev-parse HEAD).Trim()
        if ($LASTEXITCODE -ne 0 -or $gitCommit -notmatch '^[0-9a-f]{40}$') {
            throw 'Could not identify the ProjectCore source commit.'
        }
        $workingTreeStatus = @(
            & git -C $script:WorkspaceRoot status --short
        )
        if ($LASTEXITCODE -ne 0) {
            throw 'Could not inspect the ProjectCore working tree.'
        }

        $report = [ordered]@{
            schemaVersion = 1
            suite = 'project_core_session_revision'
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
                    Get-FileHash `
                        -LiteralPath $executablePath `
                        -Algorithm SHA256
                ).Hash.ToLowerInvariant()
                buildInputFileCount = $finalBuildInputState.fileCount
                buildInputDigestSha256 = $finalBuildInputState.digestSha256
                buildInputsDirty = $false
                workingTreeDirty = $workingTreeStatus.Count -gt 0
            }
            checks = @($checks)
            results = [ordered]@{
                runId = $runId
                probeRoot = $probeRoot
                prepared = $prepared
                ready = $ready
                completed = $completed
                inputsBefore = @($inputsBefore)
                inputAccessDenied = @($lockChecks)
                inputsAfter = @($inputsAfter)
                outputs = @($outputs)
            }
            claims = [ordered]@{
                headlessBeforeProjectHost = $true
                editableProjectOwned = $false
                persistedRevisionInput = $true
                completeBatchItemCount = 2
                projectInputWriteAccessRequired = $false
                projectInputDeleteAccessRequired = $false
                projectInputsUnchanged = $true
            }
        }
    }
    finally {
        $cleanupErrors = [System.Collections.Generic.List[string]]::new()
        foreach ($stream in @($inputStreams)) {
            try {
                $stream.Dispose()
                [void] $inputStreams.Remove($stream)
            }
            catch {
                $cleanupErrors.Add($_.Exception.Message)
            }
        }
        foreach ($process in @($startedProcesses)) {
            try {
                Stop-OwnedProjectCoreProcess -Process $process
            }
            catch {
                $cleanupErrors.Add($_.Exception.Message)
            }
        }
        if ($locationWasPushed) {
            try {
                Pop-Location
                $locationWasPushed = $false
            }
            catch {
                $cleanupErrors.Add($_.Exception.Message)
            }
        }
        try {
            Set-ProjectCoreRunnerEnvironmentValue `
                -Name 'CARGO_TARGET_DIR' `
                -Value $previousCargoTarget
        }
        catch {
            $cleanupErrors.Add($_.Exception.Message)
        }
        try {
            Set-ProjectCoreRunnerEnvironmentValue `
                -Name 'CARGO_BUILD_JOBS' `
                -Value $previousCargoBuildJobs
        }
        catch {
            $cleanupErrors.Add($_.Exception.Message)
        }
        if ($cleanupErrors.Count -gt 0) {
            throw ('ProjectCore cleanup failed: ' + ($cleanupErrors -join ' | '))
        }
    }

    if ($null -eq $report) {
        throw 'The ProjectCore gate did not produce a report.'
    }
    $outputDirectory = Split-Path -Parent $OutputPath
    New-Item -ItemType Directory -Force -Path $outputDirectory | Out-Null
    $json = $report | ConvertTo-Json -Depth 12
    [System.IO.File]::WriteAllText(
        $OutputPath,
        $json + [System.Environment]::NewLine,
        [System.Text.UTF8Encoding]::new($false)
    )
    Write-Output "ProjectCore report: $OutputPath"
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
