param(
    [string] $Root,
    [string] $OutputPath
)

$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'Local-Toolchain.ps1')
Initialize-MyAlbunsToolchain

if ([string]::IsNullOrWhiteSpace($Root)) {
    $Root = Join-Path $script:WorkspaceRoot 'benchmark-data\albums'
}
elseif (-not [System.IO.Path]::IsPathRooted($Root)) {
    $Root = Join-Path $script:WorkspaceRoot $Root
}
if ([string]::IsNullOrWhiteSpace($OutputPath)) {
    $OutputPath = Join-Path $script:WorkspaceRoot '.scratch\topology-corpus\manifest.json'
}
elseif (-not [System.IO.Path]::IsPathRooted($OutputPath)) {
    $OutputPath = Join-Path $script:WorkspaceRoot $OutputPath
}

$rootItem = Get-Item -LiteralPath $Root
if (-not $rootItem.PSIsContainer) {
    throw 'A raiz do corpus não é uma pasta.'
}
$albums = @(Get-ChildItem -LiteralPath $rootItem.FullName -Directory | Sort-Object Name)
if ($albums.Count -ne 2) {
    throw "O corpus precisa conter exatamente duas pastas de Álbum; foram encontradas $($albums.Count)."
}

Add-Type -AssemblyName System.Drawing

function Get-JpegMetadata {
    param([Parameter(Mandatory = $true)][System.IO.FileInfo] $File)

    $stream = $null
    $image = $null
    try {
        $stream = [System.IO.File]::Open(
            $File.FullName,
            [System.IO.FileMode]::Open,
            [System.IO.FileAccess]::Read,
            [System.IO.FileShare]::Read
        )
        $image = [System.Drawing.Image]::FromStream($stream, $false, $false)
        $orientation = 1
        if ($image.PropertyIdList -contains 0x0112) {
            $orientationBytes = $image.GetPropertyItem(0x0112).Value
            if ($orientationBytes.Length -ge 2) {
                $orientation = [System.BitConverter]::ToUInt16(
                    $orientationBytes,
                    0
                )
            }
        }
        return [ordered]@{
            width = $image.Width
            height = $image.Height
            orientation = $orientation
        }
    }
    finally {
        if ($null -ne $image) {
            $image.Dispose()
        }
        if ($null -ne $stream) {
            $stream.Dispose()
        }
    }
}

$manifestAlbums = [System.Collections.Generic.List[object]]::new()
$fingerprintRows = [System.Collections.Generic.List[string]]::new()
$totalFiles = 0
$totalBytes = [long] 0

for ($albumIndex = 0; $albumIndex -lt $albums.Count; $albumIndex += 1) {
    $album = $albums[$albumIndex]
    $slot = if ($albumIndex -eq 0) { 'a' } else { 'b' }
    $allFiles = @(Get-ChildItem -LiteralPath $album.FullName -File -Recurse)
    $unsupported = @(
        $allFiles | Where-Object {
            $_.Extension -notin @('.jpg', '.jpeg', '.JPG', '.JPEG')
        }
    )
    if ($unsupported.Count -gt 0) {
        throw (
            "O Álbum $slot contém $($unsupported.Count) arquivo(s) " +
            'fora dos formatos JPEG aceitos pelo corpus.'
        )
    }
    $files = @($allFiles | Sort-Object FullName)
    if ($files.Count -eq 0) {
        throw "O Álbum $slot não contém Fotos JPEG."
    }

    $photos = [System.Collections.Generic.List[object]]::new()
    for ($photoIndex = 0; $photoIndex -lt $files.Count; $photoIndex += 1) {
        $file = $files[$photoIndex]
        $metadata = Get-JpegMetadata -File $file
        $sha256 = (
            Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256
        ).Hash.ToLowerInvariant()
        $mediaId = 'benchmark-{0}-{1:d3}' -f $slot, ($photoIndex + 1)
        $photos.Add([ordered]@{
            mediaId = $mediaId
            name = $file.Name
            sourcePath = $file.FullName
            sourceWidthPx = $metadata.width
            sourceHeightPx = $metadata.height
            sourceBytes = $file.Length
            sourceSha256 = $sha256
            orientation = $metadata.orientation
        })
        $fingerprintRows.Add(
            "$slot`0$mediaId`0$sha256`0$($file.Length)`0" +
            "$($metadata.width)x$($metadata.height)`0$($metadata.orientation)"
        )
        $totalFiles += 1
        $totalBytes += $file.Length
    }

    $manifestAlbums.Add([ordered]@{
        slot = $slot
        name = $album.Name
        directory = $album.FullName
        photos = $photos
    })
}

$fingerprintPayload = [System.Text.Encoding]::UTF8.GetBytes(
    $fingerprintRows -join "`n"
)
$fingerprintAlgorithm = [System.Security.Cryptography.SHA256]::Create()
try {
    $corpusSha256 = -join (
        $fingerprintAlgorithm.ComputeHash($fingerprintPayload) |
            ForEach-Object { $_.ToString('x2') }
    )
}
finally {
    $fingerprintAlgorithm.Dispose()
}

$manifest = [ordered]@{
    schemaVersion = 1
    generatedAtUtc = [System.DateTime]::UtcNow.ToString('o')
    root = $rootItem.FullName
    corpusSha256 = $corpusSha256
    totalFiles = $totalFiles
    totalBytes = $totalBytes
    albums = $manifestAlbums
}

$outputDirectory = Split-Path -Parent $OutputPath
[System.IO.Directory]::CreateDirectory($outputDirectory) | Out-Null
[System.IO.File]::WriteAllText(
    [System.IO.Path]::GetFullPath($OutputPath),
    ($manifest | ConvertTo-Json -Depth 8) + [System.Environment]::NewLine,
    [System.Text.UTF8Encoding]::new($false)
)

[pscustomobject]@{
    Manifest = [System.IO.Path]::GetFullPath($OutputPath)
    Albums = $manifestAlbums.Count
    Photos = $totalFiles
    TotalMiB = [Math]::Round($totalBytes / 1MB, 1)
    CorpusSha256 = $corpusSha256
} | Format-List
