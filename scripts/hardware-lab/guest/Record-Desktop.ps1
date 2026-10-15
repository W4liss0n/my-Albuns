# Saves what the interactive desktop shows, one JPEG per interval, for the
# seconds set in C:\Lab\record.json. Runs as an Interactive scheduled task:
# the PowerShell Direct session lives in session 0 and sees no desktop. Each
# file is named by the milliseconds since the recording started.
$ErrorActionPreference = 'Stop'
$settings = Get-Content -LiteralPath 'C:\Lab\record.json' -Raw -Encoding UTF8 | ConvertFrom-Json
Add-Type -AssemblyName System.Drawing, System.Windows.Forms
New-Item -ItemType Directory -Path $settings.directory -Force | Out-Null

$bounds = [Windows.Forms.SystemInformation]::VirtualScreen
$encoder = [Drawing.Imaging.ImageCodecInfo]::GetImageEncoders() | Where-Object { $_.MimeType -eq 'image/jpeg' }
$quality = [Drawing.Imaging.EncoderParameters]::new(1)
$quality.Param[0] = [Drawing.Imaging.EncoderParameter]::new([Drawing.Imaging.Encoder]::Quality, [long] 70)
$clock = [Diagnostics.Stopwatch]::StartNew()
@{ startedAt = [DateTime]::UtcNow.ToString('o') } | ConvertTo-Json |
    Set-Content -LiteralPath (Join-Path $settings.directory 'recording.json') -Encoding UTF8
while ($clock.Elapsed.TotalSeconds -lt $settings.seconds) {
    $bitmap = [Drawing.Bitmap]::new($bounds.Width, $bounds.Height)
    try {
        $graphics = [Drawing.Graphics]::FromImage($bitmap)
        try {
            $graphics.CopyFromScreen($bounds.Left, $bounds.Top, 0, 0, $bitmap.Size)
        }
        finally {
            $graphics.Dispose()
        }
        $name = '{0:D6}.jpg' -f [int] $clock.Elapsed.TotalMilliseconds
        $bitmap.Save((Join-Path $settings.directory $name), $encoder, $quality)
    }
    catch {
        # The secure desktop or a display change can refuse one capture.
    }
    finally {
        $bitmap.Dispose()
    }
    Start-Sleep -Milliseconds $settings.intervalMs
}
