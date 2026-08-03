$ErrorActionPreference = 'Stop'
$workspaceRoot = Split-Path -Parent $PSScriptRoot
$temporaryRoot = Join-Path `
    ([System.IO.Path]::GetTempPath()) `
    ("myalbuns-contracts-{0}" -f [System.Guid]::NewGuid().ToString('N'))

function Get-RelativeFilePaths {
    param(
        [Parameter(Mandatory = $true)]
        [string] $Root
    )

    $resolvedRoot = [System.IO.Path]::GetFullPath($Root).TrimEnd('\', '/')
    $rootPrefix = $resolvedRoot + [System.IO.Path]::DirectorySeparatorChar

    @(
        Get-ChildItem -LiteralPath $resolvedRoot -File -Recurse |
            ForEach-Object {
                $_.FullName.Substring($rootPrefix.Length).Replace('\', '/')
            } |
            Sort-Object
    )
}

function Test-FilesEqual {
    param(
        [Parameter(Mandatory = $true)]
        [string] $Left,

        [Parameter(Mandatory = $true)]
        [string] $Right
    )

    (Get-FileHash -LiteralPath $Left -Algorithm SHA256).Hash -eq
        (Get-FileHash -LiteralPath $Right -Algorithm SHA256).Hash
}

function Compare-GeneratedContract {
    param(
        [Parameter(Mandatory = $true)]
        [string] $GeneratedRoot,

        [Parameter(Mandatory = $true)]
        [string] $CheckedInRoot,

        [Parameter(Mandatory = $true)]
        [string] $ContractName
    )

    $generatedFiles = @(Get-RelativeFilePaths -Root $GeneratedRoot)
    $checkedInFiles = @(Get-RelativeFilePaths -Root $CheckedInRoot)
    $problems = [System.Collections.Generic.List[string]]::new()

    foreach ($relativePath in $generatedFiles) {
        if ($relativePath -notin $checkedInFiles) {
            $problems.Add("${ContractName}: missing: $relativePath")
            continue
        }

        $generatedPath = Join-Path $GeneratedRoot $relativePath
        $checkedInPath = Join-Path $CheckedInRoot $relativePath
        if (-not (Test-FilesEqual -Left $generatedPath -Right $checkedInPath)) {
            $problems.Add("${ContractName}: changed: $relativePath")
        }
    }

    foreach ($relativePath in $checkedInFiles) {
        if ($relativePath -notin $generatedFiles) {
            $problems.Add("${ContractName}: stale: $relativePath")
        }
    }

    $problems
}

$failure = $null
New-Item -ItemType Directory -Path $temporaryRoot | Out-Null
try {
    $temporaryDomainRoot = Join-Path $temporaryRoot 'domain'
    $temporaryIpcRoot = Join-Path $temporaryRoot 'ipc'
    & (Join-Path $PSScriptRoot 'Invoke-LocalCargo.ps1') `
        run `
        -p myalbuns-core `
        --example generate_project_contract `
        -- $temporaryDomainRoot
    if ($LASTEXITCODE -ne 0) {
        throw "Generating the domain TypeScript bindings failed with exit code $LASTEXITCODE."
    }

    & (Join-Path $PSScriptRoot 'Invoke-LocalCargo.ps1') `
        run `
        -p myalbuns-desktop `
        --example generate_ipc_contract `
        -- $temporaryIpcRoot
    if ($LASTEXITCODE -ne 0) {
        throw "Generating the IPC TypeScript bindings failed with exit code $LASTEXITCODE."
    }

    $problems = @(
        Compare-GeneratedContract `
            -GeneratedRoot $temporaryDomainRoot `
            -CheckedInRoot (Join-Path $workspaceRoot 'src/domain/generated') `
            -ContractName 'domain'
        Compare-GeneratedContract `
            -GeneratedRoot $temporaryIpcRoot `
            -CheckedInRoot (Join-Path $workspaceRoot 'src/platform/generated') `
            -ContractName 'ipc'
    )

    if ($problems.Count -gt 0) {
        $failure = "The TypeScript bindings generated from the Rust contracts are out of date.`n - " +
            ($problems -join "`n - ")
    }
}
finally {
    Remove-Item -LiteralPath $temporaryRoot -Recurse -Force
}

if ($null -ne $failure) {
    Write-Error $failure
    exit 1
}

Write-Host 'The generated domain and IPC TypeScript bindings are up to date.'
