$ErrorActionPreference = 'Stop'
$workspaceRoot = Split-Path -Parent $PSScriptRoot

& (Join-Path $PSScriptRoot 'Generate-ProjectContract.ps1')
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

$status = & git -C $workspaceRoot status `
    --porcelain `
    --untracked-files=all `
    -- src/domain/generated
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}
$outOfDate = @(
    $status | Where-Object {
        $_.StartsWith('??') -or ($_.Length -gt 1 -and $_[1] -ne ' ')
    }
)
if ($outOfDate.Count -gt 0) {
    Write-Error "The TypeScript bindings generated from the Rust contract are out of date.`n$outOfDate"
    exit 1
}
