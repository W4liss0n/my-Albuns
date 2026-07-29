$ErrorActionPreference = 'Stop'

& (Join-Path $PSScriptRoot 'Invoke-LocalCargo.ps1') `
    run `
    -p myalbuns-core `
    --example generate_project_contract `
    -- src/domain/generated
exit $LASTEXITCODE
