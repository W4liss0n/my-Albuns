$ErrorActionPreference = 'Stop'
$workspaceRoot = Split-Path -Parent $PSScriptRoot
$checkedInRoot = Join-Path $workspaceRoot 'src/domain/generated'
$temporaryRoot = Join-Path `
    ([System.IO.Path]::GetTempPath()) `
    ("myalbuns-project-contract-{0}" -f [System.Guid]::NewGuid().ToString('N'))

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

$failure = $null
New-Item -ItemType Directory -Path $temporaryRoot | Out-Null
try {
    & (Join-Path $PSScriptRoot 'Invoke-LocalCargo.ps1') `
        run `
        -p myalbuns-core `
        --example generate_project_contract `
        -- $temporaryRoot
    if ($LASTEXITCODE -ne 0) {
        throw "Generating the TypeScript bindings failed with exit code $LASTEXITCODE."
    }

    $generatedFiles = @(Get-RelativeFilePaths -Root $temporaryRoot)
    $checkedInFiles = @(Get-RelativeFilePaths -Root $checkedInRoot)
    $problems = [System.Collections.Generic.List[string]]::new()

    foreach ($relativePath in $generatedFiles) {
        if ($relativePath -notin $checkedInFiles) {
            $problems.Add("missing: $relativePath")
            continue
        }

        $generatedPath = Join-Path $temporaryRoot $relativePath
        $checkedInPath = Join-Path $checkedInRoot $relativePath
        if (-not (Test-FilesEqual -Left $generatedPath -Right $checkedInPath)) {
            $problems.Add("changed: $relativePath")
        }
    }

    foreach ($relativePath in $checkedInFiles) {
        if ($relativePath -notin $generatedFiles) {
            $problems.Add("stale: $relativePath")
        }
    }

    if ($problems.Count -gt 0) {
        $failure = "The TypeScript bindings generated from the Rust contract are out of date.`n - " +
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

Write-Host 'The generated TypeScript bindings are up to date.'
