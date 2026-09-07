. (Join-Path $PSScriptRoot 'Gate-SourceProvenance.ps1')

function Get-NativeGateArtifact {
    param([string] $Path)
    $file = Get-Item -LiteralPath $Path -ErrorAction Stop
    if ($file.PSIsContainer) { throw "Expected a native gate executable: $Path" }
    return [ordered]@{
        path = $file.FullName
        sha256 = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
        length = [long] $file.Length
        lastWriteUtc = $file.LastWriteTimeUtc.ToString('o')
    }
}

function Assert-NativeGateBuildSource {
    param([psobject] $Build, [psobject] $Source)
    if ($Source.sourceInputsDirty -or $Build.gitCommit -cne $Source.gitCommit) {
        throw 'The native build requires the same clean source commit. Rebuild before running a native scenario.'
    }
}

function Read-NativeGateBuild {
    param([string] $ManifestPath, [string] $WorkspaceRoot)
    $manifest = Get-Content -LiteralPath $ManifestPath -Raw | ConvertFrom-Json
    $current = Get-GateSourceSnapshot -WorkspaceRoot $WorkspaceRoot -EvidencePath $ManifestPath
    if ($manifest.schemaVersion -ne 1 -or
        $manifest.buildMode -cne 'tauri-debug-custom-protocol' -or
        $manifest.sourceInputsDirty -ne $false) {
        throw 'The native build requires the same clean source commit. Run npm run build:native-tests after committing the source changes.'
    }
    Assert-NativeGateBuildSource -Build $manifest -Source $current
    foreach ($name in @('application', 'fixture', 'processor')) {
        $expected = $manifest.$name
        if ($null -eq $expected -or [string]::IsNullOrWhiteSpace($expected.path)) {
            throw "Missing native build artifact: $name"
        }
        $actual = Get-NativeGateArtifact -Path $expected.path
        if ($actual.sha256 -cne $expected.sha256 -or $actual.length -ne $expected.length) {
            throw "The native build artifact changed: $name. Rebuild the native test bundle."
        }
    }
    return $manifest
}
