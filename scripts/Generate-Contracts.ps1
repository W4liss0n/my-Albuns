$ErrorActionPreference = 'Stop'

& (Join-Path $PSScriptRoot 'Invoke-LocalCargo.ps1') `
    run `
    -p myalbuns-core `
    --example generate_project_contract `
    -- src/domain/generated
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

& (Join-Path $PSScriptRoot 'Invoke-LocalCargo.ps1') `
    run `
    -p myalbuns-desktop `
    --example generate_ipc_contract `
    -- src/platform/generated
exit $LASTEXITCODE
