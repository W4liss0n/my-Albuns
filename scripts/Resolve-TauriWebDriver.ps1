$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'Local-Toolchain.ps1')
Initialize-MyAlbunsToolchain

$webViewRoot = Join-Path ${env:ProgramFiles(x86)} 'Microsoft\EdgeWebView\Application'
if (-not (Test-Path -LiteralPath $webViewRoot -PathType Container)) {
    throw 'The Evergreen WebView2 runtime directory was not found.'
}
$runtime = Get-ChildItem -LiteralPath $webViewRoot -Directory |
    Where-Object {
        $parsed = $null
        [version]::TryParse($_.Name, [ref]$parsed) -and
        (Test-Path -LiteralPath (Join-Path $_.FullName 'msedgewebview2.exe') -PathType Leaf)
    } |
    Sort-Object { [version]$_.Name } -Descending |
    Select-Object -First 1
if ($null -eq $runtime) {
    throw 'No executable Evergreen WebView2 runtime version was found.'
}
$runtimeVersion = $runtime.Name

$tauriDriverVersion = '2.0.6'
$tauriDriverRoot = Join-Path $script:WorkspaceRoot ".tools\tauri-driver\$tauriDriverVersion"
$tauriDriver = Join-Path $tauriDriverRoot 'bin\tauri-driver.exe'
if (-not (Test-Path -LiteralPath $tauriDriver -PathType Leaf)) {
    & $script:CargoExecutable `
        install `
        tauri-driver `
        --version $tauriDriverVersion `
        --locked `
        --root $tauriDriverRoot
    if ($LASTEXITCODE -ne 0) {
        throw "tauri-driver $tauriDriverVersion could not be installed."
    }
}
if (-not (Test-Path -LiteralPath $tauriDriver -PathType Leaf)) {
    throw "tauri-driver $tauriDriverVersion was not materialized."
}
$installedTauriDriver = @(
    & $script:CargoExecutable install --list --root $tauriDriverRoot 2>&1
)
if ($LASTEXITCODE -ne 0) {
    throw "The installed tauri-driver package metadata could not be read: $installedTauriDriver"
}
$expectedTauriDriverHeader = "tauri-driver v${tauriDriverVersion}:"
if (-not ($installedTauriDriver | Where-Object {
            $_.ToString().Trim() -eq $expectedTauriDriverHeader
        })) {
    throw "The executable is not owned by the reported $expectedTauriDriverHeader installation: $installedTauriDriver"
}

$nativeDriverRoot = Join-Path `
    $script:WorkspaceRoot `
    ".tools\webview2-driver\$runtimeVersion"
$nativeDriver = Join-Path $nativeDriverRoot 'msedgedriver.exe'
if (-not (Test-Path -LiteralPath $nativeDriver -PathType Leaf)) {
    New-Item -ItemType Directory -Force -Path $nativeDriverRoot | Out-Null
    $archive = Join-Path $nativeDriverRoot 'edgedriver_win64.zip'
    Invoke-WebRequest `
        -UseBasicParsing `
        -Uri "https://msedgedriver.microsoft.com/$runtimeVersion/edgedriver_win64.zip" `
        -OutFile $archive
    Expand-Archive -LiteralPath $archive -DestinationPath $nativeDriverRoot -Force
}
if (-not (Test-Path -LiteralPath $nativeDriver -PathType Leaf)) {
    throw "EdgeDriver $runtimeVersion was not materialized."
}

$nativeVersionOutput = (& $nativeDriver --version).Trim()
if ($LASTEXITCODE -ne 0 -or $nativeVersionOutput -notmatch [regex]::Escape($runtimeVersion)) {
    throw "EdgeDriver '$nativeVersionOutput' does not match WebView2 $runtimeVersion."
}

[ordered]@{
    tauriDriverPath = [System.IO.Path]::GetFullPath($tauriDriver)
    tauriDriverVersion = $tauriDriverVersion
    nativeDriverPath = [System.IO.Path]::GetFullPath($nativeDriver)
    nativeDriverVersion = $runtimeVersion
    webView2RuntimePath = Join-Path $runtime.FullName 'msedgewebview2.exe'
    webView2RuntimeVersion = $runtimeVersion
} | ConvertTo-Json -Compress
